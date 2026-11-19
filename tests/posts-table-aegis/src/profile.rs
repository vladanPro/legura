use aegis::{BrowserOptions, BrowserPage};
use std::path::Path;
use std::time::{Duration, Instant};

pub(super) struct ProfileContext<'a> {
    pub endpoint: &'a str,
    pub base_url: &'a str,
    pub options: BrowserOptions,
}

pub(super) fn check_own_profile(
    context: &ProfileContext<'_>,
    email: &str,
    password: &str,
    role: &str,
    screenshots: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    // A separate WebDriver session keeps the manager's account signed in.
    let mut member = BrowserPage::connect(context.endpoint, context.options.clone())?;
    let result = exercise(
        &mut member,
        context.base_url,
        email,
        password,
        role,
        screenshots,
    );
    let closed = member.close();
    let password = result?;
    closed?;
    Ok(password)
}

fn exercise(
    page: &mut BrowserPage,
    base_url: &str,
    email: &str,
    password: &str,
    role: &str,
    screenshots: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let profile = format!("{base_url}/admin/profile");
    let login = format!("{base_url}/login");
    let landing = if role == "editor" {
        format!("{base_url}/admin/posts")
    } else {
        profile.clone()
    };
    let replacement = format!("updated-{role}-{password}");
    page.try_goto(&login)?;
    page.type_text("#email", email)?;
    page.type_text("#password", password)?;
    page.try_click("form[action*='name=SignIn'] button[type='submit']")?;
    super::wait_for_url(page, &landing)?;
    page.try_click("nav[aria-label='Administration'] a[href='/admin/profile']")?;
    super::wait_for_url(page, &profile)?;
    page.expect_text(email).expect_text("Change password");
    assert_eq!(
        page.evaluate(
            "const nav = document.querySelector('nav[aria-label=Administration]'); return [...nav.querySelectorAll('a[data-active=true]')].map(a => a.getAttribute('href')).join('|')",
            &[],
        )?,
        "/admin/profile"
    );
    assert_eq!(
        page.evaluate(
            "const nav = document.querySelector('nav[aria-label=Administration]'); return [...nav.querySelectorAll('a')].some(a => ['/admin', '/admin/users', '/admin/settings', '/admin/media', '/admin/language', '/admin/categories'].includes(a.getAttribute('href')))",
            &[],
        )?,
        false
    );
    if role == "subscriber" {
        assert_eq!(
            page.evaluate(
                "return !!document.querySelector('nav[aria-label=Administration] a[href=\"/admin/posts\"], nav[aria-label=Administration] a[href=\"/admin/pages\"]')",
                &[],
            )?,
            false
        );
    }
    for width in [320, 390] {
        page.set_viewport(width, 844)?;
        assert_eq!(
            page.evaluate(
                "return document.documentElement.scrollWidth <= innerWidth",
                &[]
            )?,
            true,
            "{role} Profile overflows at {width}px"
        );
        std::fs::write(
            screenshots.join(format!("profile-{role}-{width}.png")),
            page.full_page_png()?,
        )?;
    }
    page.set_viewport(1280, 900)?;
    fill_passwords(page, "incorrect", &replacement)?;
    page.try_click("form[action*='name=ChangeProfilePassword'] button[type='submit']")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while page.evaluate(
        "return document.querySelector('[data-ax-field-error=currentPassword]')?.textContent.includes('Current password is incorrect')",
        &[],
    )? != true {
        if Instant::now() >= deadline {
            return Err("Own Profile did not show wrong-current-password feedback".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    fill_passwords(page, password, &replacement)?;
    page.try_click("form[action*='name=ChangeProfilePassword'] button[type='submit']")?;
    super::wait_for_url(page, &login)?;
    page.try_goto(&profile)?;
    assert_eq!(
        page.evaluate(
            "return performance.getEntriesByType('navigation')[0]?.responseStatus",
            &[]
        )?,
        403,
        "old Profile session survived password change"
    );
    page.try_goto(&login)?;
    page.type_text("#email", email)?;
    page.type_text("#password", &replacement)?;
    page.try_click("form[action*='name=SignIn'] button[type='submit']")?;
    super::wait_for_url(page, &landing)?;
    page.try_click("nav[aria-label='Administration'] a[href='/admin/profile']")?;
    super::wait_for_url(page, &profile)?;
    page.expect_text(email);
    page.try_click("form[action*='name=SignOut'] button[type='submit']")?;
    super::wait_for_url(page, &login)?;
    println!("Aegis: {role} own Profile, password change, fresh login and mobile layout passed");
    Ok(replacement)
}

fn fill_passwords(
    page: &BrowserPage,
    current: &str,
    replacement: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for (selector, value) in [
        ("#currentPassword", current),
        ("#new-password", replacement),
        ("#confirmPassword", replacement),
    ] {
        page.clear_text(selector)?;
        page.type_text(selector, value)?;
    }
    Ok(())
}
