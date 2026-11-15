use aegis::{Browser, BrowserOptions, BrowserPage, Key};
use std::time::{Duration, Instant};

const TABLE: &str = "[role='region'][aria-label='Scrollable posts table']";
const INSPECT_TABLE: &str = r#"
const table = document.querySelector(arguments[0]);
return table && {
  viewport: innerWidth,
  pageFits: document.documentElement.scrollWidth <= innerWidth,
  overflows: table.scrollWidth > table.clientWidth,
  focused: table === document.activeElement,
  offset: table.scrollLeft
};
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("AEGIS_WEBDRIVER_URL")?;
    let base_url = std::env::args().nth(1).ok_or("expected fixture URL")?;
    let port = base_url
        .strip_prefix("http://127.0.0.1:")
        .ok_or("expected a local IPv4 HTTP fixture")?;
    if port.parse::<u16>().ok().filter(|port| *port > 0).is_none() {
        return Err("expected a numeric fixture port".into());
    }
    let cookie = std::env::var("LEGURA_AEGIS_FIXTURE_COOKIE")?;
    let (name, value) = cookie
        .split_once('=')
        .ok_or("missing fixture session cookie")?;
    let browser = match std::env::var("AEGIS_TEST_BROWSER").as_deref() {
        Ok("edge") => Browser::Edge,
        Ok("chrome") | Err(std::env::VarError::NotPresent) => Browser::Chrome,
        _ => return Err("AEGIS_TEST_BROWSER must be chrome or edge".into()),
    };
    for javascript in [true, false] {
        let mut page = BrowserPage::connect(
            &endpoint,
            BrowserOptions {
                browser,
                javascript,
                ..BrowserOptions::default()
            },
        )?;
        page.try_goto(&format!("{base_url}/login"))?;
        page.add_session_cookie(name, value)?;
        page.set_viewport(320, 844)?;
        page.goto(&format!("{base_url}/admin/posts"))
            .expect_text("Aegis table draft");
        let args = [TABLE.into()];
        page.evaluate(
            "const table = document.querySelector(arguments[0]); if (table) { table.focus(); table.scrollLeft = 0; }",
            &args,
        )?;
        let start = page.evaluate(INSPECT_TABLE, &args)?;
        assert_eq!(
            start["viewport"], 320,
            "fixture did not use mobile viewport"
        );
        assert_eq!(start["pageFits"], true, "table expands the page");
        assert_eq!(
            start["overflows"], true,
            "wide table must scroll inside its region"
        );
        assert_eq!(start["focused"], true, "table region must accept focus");
        let offset = start["offset"].as_f64().ok_or("missing scroll offset")?;
        page.press(Key::ArrowRight)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let state = page.evaluate(INSPECT_TABLE, &args)?;
            if state["focused"] == true
                && state["offset"].as_f64().is_some_and(|left| left > offset)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "focused table did not scroll with ArrowRight (page scripts: {javascript})"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        page.close()?;
        println!("Aegis: Legura Posts table scrolls at 320 px (page scripts: {javascript})");
    }
    Ok(())
}
