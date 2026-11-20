use super::profile::ProfileContext;
use aegis::BrowserPage;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn check_categories(
    page: &mut BrowserPage,
    mode: &str,
    screenshots: &Path,
    context: &ProfileContext<'_>,
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
    check_public_archive(page, &renamed, screenshots, context)?;
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
    check_post_filter(page, screenshots)?;
    Ok(())
}

fn check_post_filter(
    page: &BrowserPage,
    screenshots: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let category = page.evaluate("return document.querySelector('form[action*=\"name=SetPostCategory\"] input[name=category_id]').value", &[])?;
    let category = category
        .as_str()
        .ok_or("picker did not expose a category ID")?;
    page.try_click("nav[aria-label='Administration'] a[href='/admin/posts']")?;
    page.try_click(&format!("#post-category option[value='{category}']"))?;
    page.try_click("#post-status option[value=draft]")?;
    page.type_text("#post-search", "Aegis table draft")?;
    page.try_click("form:has(#post-search) button[type=submit]")?;
    wait_for(page, "const q = new URL(location.href).searchParams; return q.get('category') === arguments[0] && q.get('status') === 'draft' && q.get('q') === 'Aegis table draft'", &[category])?;
    assert_eq!(
        page.evaluate(
            "return document.querySelectorAll('table tbody tr').length",
            &[]
        )?,
        1
    );
    assert_eq!(
        page.evaluate("return document.querySelector('#post-category').value", &[])?,
        category
    );
    for width in [320, 390] {
        page.set_viewport(width, 844)?;
        assert_eq!(
            page.evaluate(
                "return document.documentElement.scrollWidth <= innerWidth",
                &[]
            )?,
            true,
            "Post filters overflow at {width}px"
        );
        std::fs::write(
            screenshots.join(format!("post-category-filter-{width}.png")),
            page.full_page_png()?,
        )?;
    }
    page.set_viewport(1280, 900)?;
    page.try_click("a.legura-filter-reset")?;
    wait_for(page, "return location.pathname === '/admin/posts' && location.search === '' && document.querySelector('#post-category')?.value === '' && document.querySelector('#post-status')?.value === 'all' && document.querySelector('#post-search')?.value === ''", &[])?;
    println!("Aegis: combined post category/status/title filter, reset and mobile layout passed");
    Ok(())
}

fn check_public_archive(
    owner: &BrowserPage,
    slug: &str,
    screenshots: &Path,
    context: &ProfileContext<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = BrowserPage::connect(context.endpoint, context.options.clone())?;
    let result = exercise_public_archive(owner, &mut reader, slug, screenshots, context.base_url);
    let closed = reader.close();
    result?;
    closed?;
    Ok(())
}

fn exercise_public_archive(
    owner: &BrowserPage,
    reader: &mut BrowserPage,
    slug: &str,
    screenshots: &Path,
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = format!("{base_url}/categories/{slug}");
    reader.try_goto(&format!("{base_url}/categories"))?;
    reader.expect_text("No published categories yet");
    reader.try_goto(&archive)?;
    assert_eq!(
        reader.evaluate(
            "return performance.getEntriesByType('navigation')[0]?.responseStatus",
            &[]
        )?,
        404
    );
    owner.try_click("a.legura-post-title")?;
    let edit = owner.current_url()?;
    owner.try_click("#status option[value=published]")?;
    owner.try_click("form[action*='name=UpdatePost'] button[type=submit]")?;
    wait_for(owner, "return location.pathname === '/admin/posts' && !!document.querySelector('table tbody') && document.querySelector('table tbody').textContent.includes('published')", &[])?;
    reader.try_goto(&format!("{base_url}/posts"))?;
    reader.try_click("a[href='/categories']")?;
    reader.try_click(&format!("a[href='/categories/{slug}']"))?;
    wait_for(reader, "return location.pathname === arguments[0] && document.querySelector('h1')?.textContent === 'Renamed category'", &[&format!("/categories/{slug}")])?;
    assert_eq!(
        reader.evaluate(
            "return !!document.querySelector('.legura-admin-shell')",
            &[]
        )?,
        false
    );
    for width in [320, 390] {
        reader.set_viewport(width, 844)?;
        assert_eq!(
            reader.evaluate(
                "return document.documentElement.scrollWidth <= innerWidth",
                &[]
            )?,
            true,
            "Public category overflow at {width}px"
        );
        std::fs::write(
            screenshots.join(format!("public-category-{width}.png")),
            reader.full_page_png()?,
        )?;
    }
    reader.set_viewport(1280, 900)?;
    reader.try_click("a[href='/posts/aegis-table-draft']")?;
    wait_for(
        reader,
        "return !!document.querySelector(arguments[0])",
        &[&format!(
            "nav[aria-label=Categories] a[href='/categories/{slug}']"
        )],
    )?;
    owner.try_goto(&edit)?;
    owner.try_click("#status option[value=draft]")?;
    owner.try_click("form[action*='name=UpdatePost'] button[type=submit]")?;
    wait_for(owner, "return location.pathname === '/admin/posts' && !!document.querySelector('table tbody') && document.querySelector('table tbody').textContent.includes('draft')", &[])?;
    reader.try_goto(&archive)?;
    assert_eq!(
        reader.evaluate(
            "return performance.getEntriesByType('navigation')[0]?.responseStatus",
            &[]
        )?,
        404
    );
    reader.expect_text("This category is not available.");
    reader.try_goto(&format!("{base_url}/categories"))?;
    reader.expect_text("No published categories yet");
    println!("Aegis: anonymous category discovery, publication/unpublication, story links and mobile public layout passed");
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
