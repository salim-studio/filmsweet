//! Community and project links (Help menu, About dialog, header Discord button, Home screen).
//!
//! FilmCraft is part of the ArtCraft family: the community lives on the ArtCraft Discord, every
//! app has a page on the ArtCraft website and a public GitHub repository.

/// The app's short name, used in the website and repository URLs.
pub const APP: &str = "filmcraft";

/// The ArtCraft community Discord.
pub const DISCORD: &str = "https://discord.gg/artcraft";
/// The ArtCraft website.
pub const WEBSITE: &str = "https://getartcraft.com";
/// FilmCraft's page on the ArtCraft website.
pub const APP_PAGE: &str = "https://getartcraft.com/apps/filmcraft";
/// FilmCraft's source repository.
pub const GITHUB: &str = "https://github.com/storytold/filmcraft";
/// New issue on FilmCraft's repository.
pub const ISSUES: &str = "https://github.com/storytold/filmcraft/issues";

/// (command id, label, url) for every link, in menu order.
pub const ALL: [(&str, &str, &str); 5] = [
    ("help.discord", "Join the ArtCraft Discord…", DISCORD),
    ("help.website", "ArtCraft Website", WEBSITE),
    ("help.appPage", "FilmCraft on getartcraft.com", APP_PAGE),
    ("help.github", "FilmCraft on GitHub", GITHUB),
    ("help.reportIssue", "Report an Issue…", ISSUES),
];

/// The URL a `help.*` link command opens.
pub fn url_for(command: &str) -> Option<&'static str> {
    ALL.iter().find(|(id, _, _)| *id == command).map(|(_, _, u)| *u)
}

/// What a Help menu item does, shown when the pointer rests on it: the address a link opens in
/// the web browser, or that the item stays inside FilmCraft.
pub fn menu_hint(command: &str) -> Option<String> {
    let url = if command == "help.filmcraftHelp" { Some(crate::panels::keyboard::HELP_URL) } else { url_for(command) };
    if let Some(url) = url {
        return Some(tlf!("Opens in your web browser: {url}", url));
    }
    Some(
        match command {
            "help.revealLogFiles" => tl!("Saves this session's log and shows it in your file manager (no web page)."),
            "help.systemCompatibilityReport" => tl!("Shows a report on this computer in a FilmCraft window (no web page)."),
            "app.about" => tl!("Shows the version, community links and credits in a FilmCraft window (no web page)."),
            _ => return None,
        }
        .to_string(),
    )
}

/// Open `url` in the system browser (a new tab on the web). The desktop host's
/// [`HostHooks::open_url`](crate::HostHooks::open_url) opens it; without the hook, or when it
/// fails, it goes through `ctx.open_url`, which only does something on the web: the native egui
/// backend is built without its `links` feature, so there it would only log (#642). When the
/// host can't open a browser, the address goes to the clipboard and the status bar says so, so
/// the click is never silently lost.
pub fn open(app: &mut crate::FilmcraftApp, ctx: &egui::Context, url: &str) {
    if let Some(f) = app.hooks.open_url.as_mut() {
        match f(url) {
            Ok(()) => return,
            Err(error) => {
                log::warn!("can't open {url} in the browser: {error}");
                ctx.copy_text(url.to_string());
                app.ui.status = tlf!("Couldn't open your web browser ({error}). The address is copied to the clipboard: {url}", error, url);
            }
        }
    }
    ctx.open_url(egui::OpenUrl::new_tab(url));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_follow_the_artcraft_scheme() {
        assert_eq!(APP_PAGE, format!("{WEBSITE}/apps/{APP}"));
        assert_eq!(GITHUB, format!("https://github.com/storytold/{APP}"));
        assert!(ALL.iter().all(|(id, _, u)| id.starts_with("help.") && u.starts_with("https://")));
        assert_eq!(url_for("help.discord"), Some(DISCORD));
        assert_eq!(url_for("help.nope"), None);
    }

    #[test]
    fn links_open_through_the_host_hook() {
        // #642: on the desktop every link (Help menu, About, Discord button, Import screen) must
        // reach the host's browser opener; `ctx.open_url` alone is a no-op natively.
        use std::cell::RefCell;
        use std::rc::Rc;
        let opened: Rc<RefCell<Vec<String>>> = Rc::default();
        let rec = opened.clone();
        let mut app = crate::FilmcraftApp::new(filmcraft_engine::Session::default());
        app.hooks.open_url = Some(Box::new(move |u: &str| {
            rec.borrow_mut().push(u.to_string());
            Ok(())
        }));
        let ctx = egui::Context::default();
        for (id, _, url) in ALL {
            let r = crate::menus::invoke(&mut app, &ctx, id, serde_json::json!({})).unwrap();
            assert_eq!(r["url"], url, "{id}");
            assert_eq!(opened.borrow().last().map(String::as_str), Some(url), "{id}");
        }
        open(&mut app, &ctx, DISCORD);
        assert_eq!(opened.borrow().last().map(String::as_str), Some(DISCORD));
        assert_eq!(opened.borrow().len(), ALL.len() + 1);

        // A failing opener falls back to `ctx.open_url` instead of losing the click.
        app.hooks.open_url = Some(Box::new(|_: &str| Err("no browser".into())));
        open(&mut app, &ctx, ISSUES);
        let fallback: Vec<String> =
            ctx.output(|o| o.commands.iter().filter_map(|c| if let egui::OutputCommand::OpenUrl(u) = c { Some(u.url.clone()) } else { None }).collect());
        assert_eq!(fallback, [ISSUES]);
        // ... and keeps the address at hand: copied, and named in the status bar with the reason
        let copied: Vec<String> =
            ctx.output(|o| o.commands.iter().filter_map(|c| if let egui::OutputCommand::CopyText(t) = c { Some(t.clone()) } else { None }).collect());
        assert_eq!(copied, [ISSUES]);
        assert!(app.ui.status.contains(ISSUES) && app.ui.status.contains("no browser"), "{}", app.ui.status);
    }

    #[test]
    fn help_menu_items_say_where_they_go() {
        // links name their address; the other Help items say they stay inside FilmCraft
        for (id, _, url) in ALL {
            let hint = menu_hint(id).unwrap();
            assert!(hint.contains(url) && hint.contains("web browser"), "{id}: {hint}");
        }
        assert!(menu_hint("help.filmcraftHelp").unwrap().contains(crate::panels::keyboard::HELP_URL));
        let help_items: Vec<String> = crate::menus::menu_items(&crate::FilmcraftApp::new(filmcraft_engine::Session::default()))
            .into_iter()
            .filter(|it| it.path.first().map(String::as_str) == Some("Help"))
            .map(|it| it.id)
            .collect();
        assert!(help_items.len() >= ALL.len() + 3, "{help_items:?}");
        for id in &help_items {
            let hint = menu_hint(id).unwrap_or_else(|| panic!("{id} has no hover hint"));
            assert!(hint.contains("web browser") || hint.contains("no web page"), "{id}: {hint}");
        }
        assert_eq!(menu_hint("edit.undo"), None);
    }
}
