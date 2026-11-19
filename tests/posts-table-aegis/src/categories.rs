use aegis::BrowserPage;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn check_categories(
    page: &mut BrowserPage,
    mode: &str,
    screenshots: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let name = format!("Aegis {mode} category");
    let slug = format!("aegis-{mode}");
    let renamed = format!("aegis-renamed-{mode}");
    page.try_click("nav[aria-label='Administration'] a[href='/admin/categories']")?;
    wait_for(
        page,
        "return !!document.querySelector('form[action*=\"name=CreateCategory\"]')",
        &[],
    )?;
    assert_eq!(page.evaluate("return document.querySelector('nav[aria-label=Administration] a[href=\"/admin/categories\"]')?.dataset.active", &[])?, "true");
    page.type_text("#category-name", &name)?;
    page.type_text("#category-slug", "Bad/slug")?;
    page.try_click("form[action*='name=CreateCategory'] button[type=submit]")?;
    wait_for(page, "return !!document.querySelector('[data-ax-field-error=slug]')?.textContent.includes('lowercase')", &[])?;
    assert_eq!(
        page.evaluate(
            "return document.querySelector('#category-name')?.value",
            &[]
        )?,
        name
    );
    page.clear_text("#category-slug")?;
    page.type_text("#category-slug", &slug)?;
    page.try_click("form[action*='name=CreateCategory'] button[type=submit]")?;
    wait_for(page, "return [...document.querySelectorAll('table tbody tr')].some(r => r.textContent.includes(arguments[0]))", &[&name])?;
    page.type_text("#category-search", &slug)?;
    page.try_click("form:has(#category-search) button[type=submit]")?;
    wait_for(
        page,
        "return new URL(location.href).searchParams.get('q') === arguments[0]",
        &[&slug],
    )?;
    for width in [320, 390] {
        page.set_viewport(width, 844)?;
        assert_eq!(
            page.evaluate(
                "return document.documentElement.scrollWidth <= innerWidth",
                &[]
            )?,
            true,
            "Categories overflows at {width}px"
        );
        std::fs::write(
            screenshots.join(format!("categories-{width}.png")),
            page.full_page_png()?,
        )?;
    }
    page.set_viewport(1280, 900)?;
    page.try_click(&format!("a[aria-label='Edit category: {name}']"))?;
    wait_for(
        page,
        "return !!document.querySelector('form[action*=\"name=UpdateCategory\"]')",
        &[],
    )?;
    page.clear_text("#category-name")?;
    page.type_text("#category-name", "Renamed category")?;
    page.clear_text("#category-slug")?;
    page.type_text("#category-slug", &renamed)?;
    page.try_click("form[action*='name=UpdateCategory'] button[type=submit]")?;
    wait_for(page, "return !!document.querySelector('form[action*=\"name=UpdateCategory\"]') && document.querySelector('#category-slug')?.getAttribute('value') === arguments[0]", &[&renamed])?;
    let category_url = page.current_url()?;
    check_assignment(page, &renamed, screenshots)?;
    page.try_goto(&category_url)?;
    page.try_click("a[href$='/delete']")?;
    wait_for(
        page,
        "return !!document.querySelector('form[action*=\"name=DeleteCategory\"]')",
        &[],
    )?;
    page.expect_text("not its posts");
    page.type_text("#confirmSlug", "wrong-slug")?;
    page.try_click("form[action*='name=DeleteCategory'] button[type=submit]")?;
    wait_for(page, "return !!document.querySelector('[data-ax-field-error=confirmSlug]')?.textContent.includes('confirm deletion')", &[])?;
    page.clear_text("#confirmSlug")?;
    page.type_text("#confirmSlug", &renamed)?;
    page.try_click("form[action*='name=DeleteCategory'] button[type=submit]")?;
    wait_for(page, "return location.pathname === '/admin/categories' && !!document.querySelector('form[action*=\"name=CreateCategory\"]')", &[])?;
    assert_eq!(page.evaluate("return [...document.querySelectorAll('table tbody tr')].some(r => r.textContent.includes(arguments[0]))", &[renamed.into()])?, false);
    println!("Aegis: Categories validation/retry, search, edit, confirmed delete and mobile layout passed ({mode})");
    Ok(())
}

fn check_assignment(
    page: &BrowserPage,
    slug: &str,
    screenshots: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    page.try_click("nav[aria-label='Administration'] a[href='/admin/posts']")?;
    page.try_click("a.legura-post-title")?;
    page.try_click("a[href^='/admin/posts/'][href$='/categories']")?;
    wait_for(
        page,
        "return !!document.querySelector('#post-category-search')",
        &[],
    )?;
    page.type_text("#post-category-search", slug)?;
    page.try_click("form:has(#post-category-search) button[type=submit]")?;
    wait_for(
        page,
        "return new URL(location.href).searchParams.get('q') === arguments[0]",
        &[slug],
    )?;
    for assigned in [true, false, true] {
        let next = if assigned { "0" } else { "1" };
        page.try_click(
            "form[aria-label='Category assignment: Renamed category'] button[type=submit]",
        )?;
        wait_for(
            page,
            "return !!document.querySelector(arguments[0])",
            &[&format!("form[aria-label='Category assignment: Renamed category'] input[name=assigned][value='{next}']")],
        )?;
        assert_eq!(
            page.evaluate("return new URL(location.href).searchParams.get('q')", &[])?,
            slug
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
            "Post categories overflow at {width}px"
        );
        std::fs::write(
            screenshots.join(format!("post-categories-{width}.png")),
            page.full_page_png()?,
        )?;
    }
    page.set_viewport(1280, 900)?;
    println!(
        "Aegis: post category add/remove, saved state, retained search and mobile layout passed"
    );
    Ok(())
}

fn wait_for(
    page: &BrowserPage,
    script: &str,
    values: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let args: Vec<_> = values.iter().map(|value| (*value).into()).collect();
    while page.evaluate(script, &args)? != true {
        if Instant::now() >= deadline {
            return Err(format!("Categories browser condition timed out: {script}").into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}
