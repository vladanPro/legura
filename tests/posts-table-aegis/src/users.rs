use aegis::BrowserPage;
use std::time::{Duration, Instant};

const FIND_ACCOUNT: &str = r#"
return [...document.querySelectorAll("table tbody tr")].some(row =>
  row.querySelector("th")?.textContent.trim() === arguments[0] &&
  row.querySelector("td")?.textContent.trim() === arguments[1]);
"#;

pub(super) fn check_users(
    page: &BrowserPage,
    mode: &str,
    test_password: &str,
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

    page.try_click(&format!("a[aria-label='View account: {email}']"))?;
    page.try_click("select[name='role'] option[value='subscriber']")?;
    page.try_click("form[action*='name=UpdateUserRole'] button[type='submit']")?;
    wait_for(
        page,
        "return [...document.querySelectorAll('dl dd')].some(value => value.textContent.trim() === 'subscriber')",
        &[],
        "account did not show the saved Subscriber role",
    )?;

    page.try_click("a.ax-link[href='/admin/users']")?;
    assert_eq!(
        page.evaluate(FIND_ACCOUNT, &[email.into(), "subscriber".into()])?,
        true
    );
    println!("Aegis: Legura Users creation, filters and role change passed ({mode})");
    Ok(())
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
