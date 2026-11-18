use aegis::{Browser, BrowserOptions, BrowserPage, Key};
use std::path::Path;
use std::time::{Duration, Instant};

mod users;

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

const INSPECT_LOGIN_ERROR: &str = r#"
const email = document.getElementById("email");
const error = document.querySelector('[data-ax-field-error="email"]');
if (!email || !error || !error.getClientRects().length ||
    getComputedStyle(error).visibility !== "visible" || !error.textContent.trim()) {
  return null;
}
return {
  message: error.textContent.trim(),
  live: error.getAttribute("aria-live"),
  invalid: email.getAttribute("aria-invalid"),
  linked: !!error.id && (email.getAttribute("aria-describedby") || "").split(/\s+/).includes(error.id),
  emailRetained: email.value === arguments[0]
};
"#;

const INSPECT_BLOCKED_SUBMISSION: &str = r#"
const email = document.getElementById("email");
return email && {
  focused: email === document.activeElement,
  valueMissing: email.validity.valueMissing,
  typeMismatch: email.validity.typeMismatch,
  submissions: window.leguraTestSubmitCount
};
"#;

const INSPECT_LOGIN_LAYOUT: &str = r#"
const email = document.getElementById("email");
const password = document.getElementById("password");
const submit = email?.form?.querySelector('button[type="submit"]');
if (!email || !password || !submit) return null;
return {
  viewport: innerWidth,
  fits: document.documentElement.scrollWidth <= innerWidth,
  targetHeight: Math.min(...[email, password, submit].map(control => control.getBoundingClientRect().height)),
  emailFontSize: parseFloat(getComputedStyle(email).fontSize)
};
"#;

fn wait_for_url(page: &BrowserPage, expected: &str) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if page.current_url()? == expected {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(format!("browser did not reach {expected}").into())
}

fn check_login_rejection(
    page: &BrowserPage,
    email: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let args = [email.into()];
    while Instant::now() < deadline {
        let error = page.evaluate(INSPECT_LOGIN_ERROR, &args)?;
        if !error.is_null() {
            assert_eq!(error["message"], "Email or password is incorrect.");
            assert_eq!(error["live"], "polite", "login error must be announced");
            assert_eq!(error["invalid"], "true", "email must be marked invalid");
            assert_eq!(error["linked"], true, "email must reference the error");
            assert_eq!(error["emailRetained"], true, "email must survive rejection");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("wrong password did not show the login field error".into())
}

fn check_mobile_login(
    page: &BrowserPage,
    screenshots: &Path,
    state: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for width in [320, 390] {
        page.set_viewport(width, 844)?;
        let layout = page.evaluate(INSPECT_LOGIN_LAYOUT, &[])?;
        assert!(!layout.is_null(), "{state}: missing login controls");
        assert_eq!(layout["viewport"], width, "{state}: unexpected viewport");
        assert_eq!(
            layout["fits"], true,
            "{state}: horizontal overflow at {width}px"
        );
        assert!(
            layout["targetHeight"]
                .as_f64()
                .is_some_and(|height| height >= 44.0),
            "{state}: login control shorter than 44px at {width}px"
        );
        assert!(
            layout["emailFontSize"]
                .as_f64()
                .is_some_and(|size| size >= 16.0),
            "{state}: email input font smaller than 16px at {width}px"
        );
        std::fs::write(
            screenshots.join(format!("{state}-{width}.png")),
            page.full_page_png()?,
        )?;
    }
    page.set_viewport(1280, 900)?;
    Ok(())
}

fn check_native_login_validation(page: &BrowserPage) -> Result<(), Box<dyn std::error::Error>> {
    page.evaluate(
        r#"
const form = document.getElementById("email").form;
window.leguraTestSubmitCount = 0;
form.addEventListener("submit", () => { window.leguraTestSubmitCount++; }, true);
"#,
        &[],
    )?;
    page.try_click("#email")?;
    page.press(Key::Tab)?;
    assert_eq!(
        page.evaluate("return document.activeElement.id", &[])?,
        "password",
        "Tab from email must focus password"
    );
    page.press(Key::Tab)?;
    let submit = page.evaluate(
        r#"
const active = document.activeElement;
const style = getComputedStyle(active);
return {
  focused: active.matches('form[action*="name=SignIn"] button[type=submit]'),
  outlineVisible: style.outlineStyle !== "none" && parseFloat(style.outlineWidth) >= 2
};
"#,
        &[],
    )?;
    assert_eq!(
        submit["focused"], true,
        "Tab from password must focus Sign in"
    );
    assert_eq!(
        submit["outlineVisible"], true,
        "Sign in focus must be visible"
    );
    page.press(Key::Enter)?;
    let empty = page.evaluate(INSPECT_BLOCKED_SUBMISSION, &[])?;
    assert_eq!(empty["focused"], true, "required email must receive focus");
    assert_eq!(empty["valueMissing"], true, "email must be required");
    assert_eq!(empty["submissions"], 0, "empty login dispatched a submit");

    page.type_text("#email", "not-an-email")?;
    page.type_text("#password", "fixture-password-placeholder")?;
    page.press(Key::Enter)?;
    let malformed = page.evaluate(INSPECT_BLOCKED_SUBMISSION, &[])?;
    assert_eq!(
        malformed["focused"], true,
        "invalid email must receive focus"
    );
    assert_eq!(
        malformed["typeMismatch"], true,
        "email format must be checked"
    );
    assert_eq!(
        malformed["submissions"], 0,
        "malformed login dispatched a submit"
    );
    page.clear_text("#email")?;
    page.clear_text("#password")?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("AEGIS_WEBDRIVER_URL")?;
    let base_url = std::env::args().nth(1).ok_or("expected fixture URL")?;
    let port = base_url
        .strip_prefix("http://127.0.0.1:")
        .ok_or("expected a local IPv4 HTTP fixture")?;
    if port.parse::<u16>().ok().filter(|port| *port > 0).is_none() {
        return Err("expected a numeric fixture port".into());
    }
    let email = std::env::var("LEGURA_AEGIS_FIXTURE_EMAIL")?;
    let password = std::env::var("LEGURA_AEGIS_FIXTURE_PASSWORD")?;
    let browser = match std::env::var("AEGIS_TEST_BROWSER").as_deref() {
        Ok("edge") => Browser::Edge,
        Ok("chrome") | Err(std::env::VarError::NotPresent) => Browser::Chrome,
        _ => return Err("AEGIS_TEST_BROWSER must be chrome or edge".into()),
    };
    let login_url = format!("{base_url}/login");
    let admin_url = format!("{base_url}/admin");
    let screenshots =
        std::env::temp_dir().join(format!("legura-aegis-auth-{}", std::process::id()));
    for javascript in [true, false] {
        let mode = if javascript { "javascript" } else { "native" };
        let output = screenshots.join(mode);
        std::fs::create_dir_all(&output)?;
        let mut page = BrowserPage::connect(
            &endpoint,
            BrowserOptions {
                browser,
                javascript,
                ..BrowserOptions::default()
            },
        )?;
        page.try_goto(&login_url)?;
        check_mobile_login(&page, &output, "login")?;
        check_native_login_validation(&page)?;
        page.type_text("#email", &email)?;
        page.type_text("#password", "wrong-fixture-password")?;
        page.press(Key::Enter)?;
        check_login_rejection(&page, &email)?;
        check_mobile_login(&page, &output, "login-validation")?;
        page.clear_text("#password")?;
        page.type_text("#password", &password)?;
        page.press(Key::Enter)?;
        wait_for_url(&page, &admin_url)?;
        page.expect_text("Signed in as").expect_text(&email);
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
        println!("Aegis: Legura Posts table scrolls at 320 px (page scripts: {javascript})");
        users::check_users(&page, mode, &format!("member-{password}"))?;
        page.try_goto(&admin_url)?;
        page.try_click("form[action*='name=SignOut'] button[type='submit']")?;
        wait_for_url(&page, &login_url)?;
        page.expect_text("Sign in to Legura");
        page.try_goto(&admin_url)?;
        let status = page.evaluate(
            "return performance.getEntriesByType('navigation')[0]?.responseStatus",
            &[],
        )?;
        assert_eq!(
            status, 403,
            "the signed-out browser must no longer reach admin"
        );
        page.close()?;
        println!("Aegis: Legura sign-in validation, retry and sign-out passed (page scripts: {javascript})");
    }
    println!("Aegis login screenshots: {}", screenshots.display());
    Ok(())
}
