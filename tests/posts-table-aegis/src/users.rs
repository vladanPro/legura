use super::profile::{check_own_profile, ProfileContext};
use aegis::BrowserPage;
use std::path::Path;
use std::time::{Duration, Instant};

const FIND_ACCOUNT: &str = r#"
return [...document.querySelectorAll("table tbody tr")].some(row =>
  row.querySelector("th")?.textContent.trim() === arguments[0] &&
  row.querySelector("td")?.textContent.trim() === arguments[1] &&
  (arguments.length < 3 || row.querySelectorAll("td")[1]?.textContent.trim() === arguments[2]));
"#;

pub(super) fn check_users(
    page: &BrowserPage,
    mode: &str,
    test_password: &str,
    screenshots: &Path,
    profile_context: &ProfileContext<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let email = format!("aegis-{mode}-user@example.test");
    page.set_viewport(1280, 900)?;
    page.try_click("nav[aria-label='Administration'] a[href='/admin/users']")?;
    page.try_click("a[href='#add-user']")?;
    page.type_text("#add-user input[name='email']", &email)?;
    page.try_click("#add-user select[name='role'] option[value='editor']")?;
    page.type_text("#add-user input[name='password']", test_password)?;
    page.try_click("#add-user button[type='submit']")?;
    wait_for(
        page,
        FIND_ACCOUNT,
        &[&email, "editor"],
        "created Editor did not appear in Users",
    )?;

    page.type_text("#user-search", &email.to_ascii_uppercase())?;
    page.try_click("#user-role option[value='editor']")?;
    page.try_click("form:has(#user-search) button[type='submit']")?;
    wait_for(
        page,
        "return new URL(location.href).searchParams.get('role') === 'editor' && document.querySelectorAll('table tbody tr').length === 1",
        &[],
        "email and role filters did not isolate the created account",
    )?;
    assert_eq!(
        page.evaluate(FIND_ACCOUNT, &[email.clone().into(), "editor".into()])?,
        true
    );

    let member_password = check_own_profile(
        profile_context,
        &email,
        test_password,
        "editor",
        screenshots,
    )?;
    page.try_click(&format!("a[aria-label='View account: {email}']"))?;
    page.try_click("select[name='role'] option[value='subscriber']")?;
    page.try_click("form[action*='name=UpdateUserRole'] button[type='submit']")?;
    wait_for(
        page,
        "return [...document.querySelectorAll('dl dd')].some(value => value.textContent.trim() === 'subscriber')",
        &[],
        "account did not show the saved Subscriber role",
    )?;

    page.try_click("form[action*='name=UpdateUserStatus'] button[type='submit']")?;
    wait_for(
        page,
        "return document.querySelector('form[action*=\"name=UpdateUserStatus\"] input[name=status]')?.value === 'active' && [...document.querySelectorAll('dl dd')].some(value => value.textContent.trim() === 'inactive')",
        &[],
        "deactivation did not show Inactive and a reactivation form",
    )?;
    page.try_click("a.ax-link[href='/admin/users']")?;
    filter_account(page, &email, "inactive")?;
    wait_for(
        page,
        FIND_ACCOUNT,
        &[&email, "subscriber", "inactive"],
        "inactive filter did not show the deactivated Subscriber",
    )?;
    for width in [320, 390] {
        page.set_viewport(width, 844)?;
        assert_eq!(
            page.evaluate(
                "return document.documentElement.scrollWidth <= innerWidth",
                &[],
            )?,
            true,
            "Users filters overflow at {width}px ({mode})"
        );
        std::fs::write(
            screenshots.join(format!("users-inactive-{width}.png")),
            page.full_page_png()?,
        )?;
    }
    page.set_viewport(1280, 900)?;

    filter_account(page, &email, "active")?;
    wait_for(
        page,
        "return document.body.textContent.includes('No matching accounts') && document.querySelectorAll('table tbody tr').length === 0",
        &[],
        "active filter incorrectly included an inactive account",
    )?;
    assert_eq!(
        page.evaluate(
            "return [...document.querySelectorAll('a[href=\"/admin/users\"]')].some(link => link.textContent.trim() === 'Reset filters')",
            &[],
        )?,
        true,
        "empty filtered listing must offer Reset filters"
    );

    filter_account(page, &email, "inactive")?;
    page.try_click(&format!("a[aria-label='View account: {email}']"))?;
    page.try_click("form[action*='name=UpdateUserStatus'] button[type='submit']")?;
    wait_for(
        page,
        "return document.querySelector('form[action*=\"name=UpdateUserStatus\"] input[name=status]')?.value === 'inactive' && [...document.querySelectorAll('dl dd')].some(value => value.textContent.trim() === 'active')",
        &[],
        "reactivation did not show Active and a deactivation form",
    )?;
    page.try_click("a.ax-link[href='/admin/users']")?;
    filter_account(page, &email, "active")?;
    assert_eq!(
        page.evaluate(
            FIND_ACCOUNT,
            &[email.clone().into(), "subscriber".into(), "active".into()]
        )?,
        true
    );
    check_own_profile(
        profile_context,
        &email,
        &member_password,
        "subscriber",
        screenshots,
    )?;
    println!("Aegis: Legura Users creation, role/status changes, filters and mobile layout passed ({mode})");
    Ok(())
}

fn filter_account(
    page: &BrowserPage,
    email: &str,
    status: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    page.clear_text("#user-search")?;
    page.type_text("#user-search", email)?;
    page.try_click("#user-role option[value='subscriber']")?;
    page.try_click(&format!("#user-status option[value='{status}']"))?;
    page.try_click("form:has(#user-search) button[type='submit']")?;
    wait_for(
        page,
        "const query = new URL(location.href).searchParams; return query.get('q') === arguments[0] && query.get('role') === 'subscriber' && query.get('status') === arguments[1] && document.querySelector('#user-status')?.value === arguments[1]",
        &[email, status],
        "combined email, role and status filters were not submitted or retained",
    )
}

fn wait_for(
    page: &BrowserPage,
    script: &str,
    args: &[&str],
    failure: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let args: Vec<_> = args.iter().map(|&arg| arg.into()).collect();
    loop {
        if page.evaluate(script, &args)? == true {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(failure.into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
