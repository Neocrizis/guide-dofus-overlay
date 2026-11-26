// Guide Dofus Overlay : affiche le journal du Guide Dofus (site en ligne, ?mode=overlay)
// dans une fenêtre sans cadre, transparente et toujours au-dessus du jeu.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ocr;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const SITE: &str = "https://neocrizis.github.io/guide-dofus/?mode=overlay";
const HOTE: &str = "neocrizis.github.io";

/// Clic au travers : la souris passe à travers la fenêtre pour jouer dessous.
static TRAVERS: AtomicBool = AtomicBool::new(false);
static DESSUS: AtomicBool = AtomicBool::new(true);

struct Menus {
    travers: CheckMenuItem<Wry>,
    dessus: CheckMenuItem<Wry>,
    chat: CheckMenuItem<Wry>,
}
/// Lien de connexion reçu avant que la page soit prête.
struct LienEnAttente(Mutex<Option<String>>);

fn journal(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("journal")
}

fn montrer(app: &AppHandle) {
    if let Some(w) = journal(app) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn basculer_visible(app: &AppHandle) {
    if let Some(w) = journal(app) {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            montrer(app);
        }
    }
}

fn regler_travers(app: &AppHandle, actif: bool) {
    TRAVERS.store(actif, Ordering::SeqCst);
    if let Some(w) = journal(app) {
        let _ = w.set_ignore_cursor_events(actif);
        let _ = w.eval(&format!(
            "document.documentElement.classList.toggle('ov-travers', {actif})"
        ));
    }
    if let Some(m) = app.try_state::<Menus>() {
        let _ = m.travers.set_checked(actif);
    }
}

/// Fait avancer (ou reculer) le guide d'une carte.
fn avancer(app: &AppHandle, sens: i32) {
    if let Some(w) = journal(app) {
        let _ = w.eval(&format!("window.GuideDofus && window.GuideDofus.avancer && window.GuideDofus.avancer({sens})"));
    }
}

fn regler_dessus(app: &AppHandle, actif: bool) {
    DESSUS.store(actif, Ordering::SeqCst);
    if let Some(w) = journal(app) {
        let _ = w.set_always_on_top(actif);
    }
    if let Some(m) = app.try_state::<Menus>() {
        let _ = m.dessus.set_checked(actif);
    }
}

/// Lecture du chat : marche / arrêt. Sans zone choisie, ouvre d'abord le choix de la zone.
fn regler_chat(app: &AppHandle, actif: bool) {
    if actif && ocr::zone().is_none() {
        ouvrir_zone(app);
        return;
    }
    ocr::regler_actif(app, actif);
    if let Some(m) = app.try_state::<Menus>() {
        let _ = m.chat.set_checked(actif);
    }
    if let Some(w) = journal(app) {
        let msg = if actif { "Lecture du chat activée" } else { "Lecture du chat désactivée" };
        let _ = w.eval(&format!("window.GuideDofus && window.GuideDofus.toast({})", serde_json::to_string(msg).unwrap()));
    }
}

/// Fenêtre plein écran, sur l'écran du journal, pour tracer la zone du chat.
fn ouvrir_zone(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("zone") {
        let _ = w.set_focus();
        return;
    }
    let ecran = journal(app).and_then(|w| w.current_monitor().ok().flatten());
    let Ok(w) = WebviewWindowBuilder::new(app, "zone", WebviewUrl::App("zone.html".into()))
        .title("Zone du chat")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()
    else {
        return;
    };
    if let Some(e) = ecran {
        let _ = w.set_position(*e.position());
        let _ = w.set_size(*e.size());
    }
    let _ = w.show();
    let _ = w.set_focus();
}

/// Reçoit le rectangle tracé (en pixels de la page) et le convertit en pixels de l'écran.
#[tauri::command]
fn definir_zone(window: WebviewWindow, x: f64, y: f64, l: f64, h: f64) {
    let app = window.app_handle().clone();
    let echelle = window.scale_factor().unwrap_or(1.0);
    let origine = window.inner_position().unwrap_or_default();
    let zone = ocr::Zone {
        x: origine.x + (x * echelle).round() as i32,
        y: origine.y + (y * echelle).round() as i32,
        l: (l * echelle).round() as i32,
        h: (h * echelle).round() as i32,
    };
    let _ = window.close();
    ocr::regler_zone(&app, zone);
    regler_chat(&app, true);
}

#[tauri::command]
fn fermer_zone(window: WebviewWindow) {
    let _ = window.close();
}

/// Transmet un lien guidedofus://auth?code=… à la page, qui termine la connexion au compte.
fn transmettre_lien(app: &AppHandle, lien: &str) {
    if !lien.starts_with("guidedofus://") {
        return;
    }
    montrer(app);
    match journal(app) {
        Some(w) => {
            let texte = serde_json::to_string(lien).unwrap_or_default();
            let _ = w.eval(&format!(
                "window.GuideDofusAuth ? window.GuideDofusAuth({texte}) : (window.__lienAuth = {texte})"
            ));
        }
        None => {
            if let Some(etat) = app.try_state::<LienEnAttente>() {
                *etat.0.lock().unwrap() = Some(lien.to_string());
            }
        }
    }
}

fn main() {
    let raccourci_visible = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyG);
    let raccourci_travers = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyT);
    // Carte suivante / précédente sans quitter le jeu.
    let raccourci_suivant = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::ArrowRight);
    let raccourci_precedent = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::ArrowLeft);

    tauri::Builder::default()
        // Une seule instance : un lien guidedofus:// ouvert depuis le navigateur arrive dans l'appli déjà lancée.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            match args.iter().find(|a| a.starts_with("guidedofus://")) {
                Some(lien) => transmettre_lien(app, lien),
                None => montrer(app),
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, raccourci, evenement| {
                    if evenement.state() != ShortcutState::Pressed {
                        return;
                    }
                    if raccourci == &raccourci_visible {
                        basculer_visible(app);
                    } else if raccourci == &raccourci_travers {
                        regler_travers(app, !TRAVERS.load(Ordering::SeqCst));
                    } else if raccourci == &raccourci_suivant {
                        avancer(app, 1);
                    } else if raccourci == &raccourci_precedent {
                        avancer(app, -1);
                    }
                })
                .build(),
        )
        .manage(LienEnAttente(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![definir_zone, fermer_zone])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Lien de connexion qui a lancé l'appli (si elle n'était pas ouverte).
            #[cfg(any(windows, target_os = "linux"))]
            {
                let _ = app.deep_link().register_all();
            }
            if let Some(lien) = std::env::args().find(|a| a.starts_with("guidedofus://")) {
                *app.state::<LienEnAttente>().0.lock().unwrap() = Some(lien);
            }
            let h = handle.clone();
            app.deep_link().on_open_url(move |ev| {
                for url in ev.urls() {
                    transmettre_lien(&h, url.as_str());
                }
            });

            let h = handle.clone();
            WebviewWindowBuilder::new(app, "journal", WebviewUrl::External(SITE.parse().unwrap()))
                .title("Guide Dofus")
                .inner_size(400.0, 640.0)
                .min_inner_size(280.0, 260.0)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .resizable(true)
                // Les liens externes (DofusDB, connexion Google…) s'ouvrent dans le navigateur du PC.
                .on_navigation(|url| {
                    let interne = url.scheme() == "https" && url.host_str() == Some(HOTE);
                    if !interne && (url.scheme() == "https" || url.scheme() == "http") {
                        let _ = open::that_detached(url.as_str());
                    }
                    interne
                })
                .on_page_load(move |w, charge| {
                    if charge.event() != PageLoadEvent::Finished {
                        return;
                    }
                    let travers = TRAVERS.load(Ordering::SeqCst);
                    let _ = w.eval(&format!(
                        "document.documentElement.classList.add('appli'); document.documentElement.classList.toggle('ov-travers', {travers})"
                    ));
                    let lien = h.state::<LienEnAttente>().0.lock().unwrap().take();
                    if let Some(lien) = lien {
                        let h2 = h.clone();
                        // Laisse le temps au script du compte de démarrer.
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            transmettre_lien(&h2, &lien);
                        });
                    }
                })
                .build()?;

            // Icône dans la zone de notification, avec son menu.
            let afficher = MenuItem::with_id(app, "afficher", "Afficher / masquer    Ctrl+Maj+G", true, None::<&str>)?;
            let travers = CheckMenuItem::with_id(app, "travers", "Clic au travers    Ctrl+Maj+T", true, false, None::<&str>)?;
            let dessus = CheckMenuItem::with_id(app, "dessus", "Toujours au premier plan", true, true, None::<&str>)?;
            let recharger = MenuItem::with_id(app, "recharger", "Recharger le guide", true, None::<&str>)?;
            let suivant = MenuItem::with_id(app, "suivant", "Carte suivante    Ctrl+Maj+→", true, None::<&str>)?;
            let precedent = MenuItem::with_id(app, "precedent", "Carte précédente    Ctrl+Maj+←", true, None::<&str>)?;
            // Lecture du chat : coupée par défaut, le joueur l'active lui-même.
            let chat_actif = ocr::charger(&handle);
            let chat = CheckMenuItem::with_id(app, "chat", "Cocher les quêtes terminées (lecture du chat)", true, chat_actif, None::<&str>)?;
            let zone = MenuItem::with_id(app, "zone", "Choisir la zone du chat…", true, None::<&str>)?;
            let sep_chat = PredefinedMenuItem::separator(app)?;
            let quitter = MenuItem::with_id(app, "quitter", "Quitter", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&afficher, &travers, &dessus, &recharger, &suivant, &precedent, &sep_chat, &chat, &zone, &sep, &quitter])?;
            app.manage(Menus { travers: travers.clone(), dessus: dessus.clone(), chat: chat.clone() });
            ocr::demarrer(handle.clone());

            TrayIconBuilder::with_id("guide")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Guide Dofus : Ctrl+Maj+G afficher, Ctrl+Maj+T clic au travers, Ctrl+Maj+→ / ← carte suivante / précédente")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "afficher" => basculer_visible(app),
                    "travers" => regler_travers(app, !TRAVERS.load(Ordering::SeqCst)),
                    "dessus" => regler_dessus(app, !DESSUS.load(Ordering::SeqCst)),
                    "recharger" => {
                        if let Some(w) = journal(app) {
                            let _ = w.eval("location.reload()");
                        }
                    }
                    "chat" => regler_chat(app, !ocr::ACTIF.load(Ordering::SeqCst)),
                    "zone" => ouvrir_zone(app),
                    "suivant" => avancer(app, 1),
                    "precedent" => avancer(app, -1),
                    "quitter" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        basculer_visible(tray.app_handle());
                    }
                })
                .build(app)?;

            app.global_shortcut().register(raccourci_visible)?;
            app.global_shortcut().register(raccourci_travers)?;
            // Un raccourci déjà pris par une autre appli ne doit pas empêcher le démarrage.
            let _ = app.global_shortcut().register(raccourci_suivant);
            let _ = app.global_shortcut().register(raccourci_precedent);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("impossible de lancer Guide Dofus Overlay");
}
