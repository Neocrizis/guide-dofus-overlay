//! Lecture du chat du jeu (désactivée par défaut).
//!
//! Toutes les 1,5 s, l'appli capture la zone de l'écran choisie par le joueur (le chat),
//! la lit avec l'OCR intégré à Windows (local, hors ligne) et transmet au guide les lignes
//! « Quête terminée : … » et « Succès déverrouillé : … ». Le guide coche alors la quête.
//! Rien n'est envoyé sur Internet, et l'appli n'agit jamais sur le jeu : elle lit l'écran, c'est tout.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Zone du chat, en pixels physiques de l'écran.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Zone {
    pub x: i32,
    pub y: i32,
    pub l: i32,
    pub h: i32,
}

#[derive(Default, Serialize, Deserialize)]
struct Reglages {
    actif: bool,
    zone: Option<Zone>,
}

pub static ACTIF: AtomicBool = AtomicBool::new(false);
static ZONE: Mutex<Option<Zone>> = Mutex::new(None);

fn fichier(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("lecture-chat.json"))
}

fn enregistrer(app: &AppHandle) {
    let r = Reglages { actif: ACTIF.load(Ordering::SeqCst), zone: *ZONE.lock().unwrap() };
    if let Some(f) = fichier(app) {
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, serde_json::to_string(&r).unwrap_or_default());
    }
}

/// Recharge les réglages au lancement. Renvoie l'état « actif ».
pub fn charger(app: &AppHandle) -> bool {
    let r: Reglages = fichier(app)
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    *ZONE.lock().unwrap() = r.zone;
    let actif = r.actif && r.zone.is_some();
    ACTIF.store(actif, Ordering::SeqCst);
    actif
}

pub fn zone() -> Option<Zone> {
    *ZONE.lock().unwrap()
}

pub fn regler_actif(app: &AppHandle, actif: bool) {
    ACTIF.store(actif, Ordering::SeqCst);
    enregistrer(app);
}

pub fn regler_zone(app: &AppHandle, z: Zone) {
    *ZONE.lock().unwrap() = Some(z);
    enregistrer(app);
}

/// Garde seulement les lignes utiles au guide (accents et majuscules ignorés).
fn utile(ligne: &str) -> bool {
    let l = ligne.to_lowercase();
    (l.contains("qu") && l.contains("termin")) || l.contains("verrouill")
}

/// Boucle de lecture, lancée une fois au démarrage ; elle ne fait rien tant que la lecture est coupée.
pub fn demarrer(app: AppHandle) {
    std::thread::spawn(move || {
        let mut lecteur = None;
        let mut deja: std::collections::VecDeque<String> = std::collections::VecDeque::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            if !ACTIF.load(Ordering::SeqCst) {
                continue;
            }
            let Some(z) = zone() else { continue };
            if lecteur.is_none() {
                lecteur = windows_ocr::Lecteur::nouveau().ok();
            }
            let Some(l) = lecteur.as_ref() else { continue };
            let Ok(lignes) = l.lire(z) else { continue };
            let nouvelles: Vec<String> = lignes
                .into_iter()
                .filter(|t| utile(t) && !deja.contains(t))
                .collect();
            if nouvelles.is_empty() {
                continue;
            }
            for t in &nouvelles {
                deja.push_back(t.clone());
                if deja.len() > 200 {
                    deja.pop_front();
                }
            }
            if let Some(w) = app.get_webview_window("journal") {
                let json = serde_json::to_string(&nouvelles).unwrap_or_default();
                let _ = w.eval(&format!("window.GuideDofusOcr && window.GuideDofusOcr({json})"));
            }
        }
    });
}

#[cfg(windows)]
mod windows_ocr {
    use super::Zone;
    use windows::core::{Result, HSTRING};
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;
    use windows::Win32::Graphics::Gdi::*;

    /// Le chat est écrit petit : l'image est agrandie avant la lecture, ce qui fiabilise l'OCR.
    const AGRANDI: i32 = 2;

    pub struct Lecteur {
        moteur: OcrEngine,
    }

    impl Lecteur {
        pub fn nouveau() -> Result<Self> {
            unsafe {
                let _ = windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);
            }
            let moteur = Language::CreateLanguage(&HSTRING::from("fr-FR"))
                .and_then(|l| OcrEngine::TryCreateFromLanguage(&l))
                .or_else(|_| OcrEngine::TryCreateFromUserProfileLanguages())?;
            Ok(Self { moteur })
        }

        pub fn lire(&self, z: Zone) -> Result<Vec<String>> {
            let max = OcrEngine::MaxImageDimension()? as i32;
            let (l, h) = ((z.l * AGRANDI).min(max), (z.h * AGRANDI).min(max));
            let pixels = capturer(z, l, h);
            if pixels.is_empty() {
                return Ok(vec![]);
            }
            let ecrivain = DataWriter::new()?;
            ecrivain.WriteBytes(&pixels)?;
            let image = SoftwareBitmap::CreateCopyFromBuffer(&ecrivain.DetachBuffer()?, BitmapPixelFormat::Bgra8, l, h)?;
            let resultat = self.moteur.RecognizeAsync(&image)?.join()?;
            let lignes = resultat.Lines()?;
            let mut textes = Vec::new();
            for i in 0..lignes.Size()? {
                textes.push(lignes.GetAt(i)?.Text()?.to_string());
            }
            Ok(textes)
        }
    }

    /// Copie de la zone de l'écran (agrandie à l x h), en BGRA de haut en bas.
    fn capturer(z: Zone, l: i32, h: i32) -> Vec<u8> {
        unsafe {
            let ecran = GetDC(None);
            let mem = CreateCompatibleDC(Some(ecran));
            let bmp = CreateCompatibleBitmap(ecran, l, h);
            let ancien = SelectObject(mem, bmp.into());
            SetStretchBltMode(mem, HALFTONE);
            let ok = StretchBlt(mem, 0, 0, l, h, Some(ecran), z.x, z.y, z.l, z.h, SRCCOPY).as_bool();
            let mut pixels = vec![0u8; (l * h * 4) as usize];
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: l,
                    biHeight: -h, // de haut en bas
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let lues = if ok {
                GetDIBits(mem, bmp, 0, h as u32, Some(pixels.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS)
            } else {
                0
            };
            SelectObject(mem, ancien);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, ecran);
            if lues == 0 {
                return vec![];
            }
            // Alpha opaque (GDI laisse 0), attendu par l'OCR.
            for px in pixels.chunks_exact_mut(4) {
                px[3] = 255;
            }
            pixels
        }
    }
}

/// Hors Windows (compilation de développement) : pas d'OCR.
#[cfg(not(windows))]
mod windows_ocr {
    use super::Zone;
    pub struct Lecteur;
    impl Lecteur {
        pub fn nouveau() -> Result<Self, ()> {
            Err(())
        }
        pub fn lire(&self, _z: Zone) -> Result<Vec<String>, ()> {
            Err(())
        }
    }
}
