$seoFixture = @'
import sqlite3, sys
d = sqlite3.connect(sys.argv[1])
with d:
    for table in ("legura_posts", "legura_pages"):
        d.execute(
            f"INSERT INTO {table}(id,title,slug,body,status) VALUES (?,?,?,?,?)",
            ("seo-story", "Fish & Chips", "seo-story", 'One "quoted"\n <line>', "published"),
        )
        d.execute(
            f"INSERT INTO {table}(id,title,slug,body,status) VALUES (?,?,?,?,?)",
            ("seo-draft", "Hidden SEO draft", "seo-draft", "Private SEO excerpt", "draft"),
        )
d.close()
'@
& $python.Source -c $seoFixture $db
if ($LASTEXITCODE -ne 0) { throw "Could not seed isolated public metadata fixtures" }

try {
  foreach ($collection in @('posts', 'pages')) {
    $public = Invoke-Request -Path "/$collection/seo-story"
    $head = $public.Body.Split('</head>')[0]
    if ([regex]::Matches($head, '<title>').Count -ne 1 -or
        !$head.Contains('<title>Fish &amp; Chips | Fixture</title>') -or
        $head -notmatch '<meta(?=[^>]*name="description")(?=[^>]*content="One &quot;quoted&quot; &lt;line&gt;")[^>]*>' -or
        $head.Contains('rel="canonical"')) {
      throw "Published $collection did not render escaped metadata without a configured address"
    }
  }

  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_general_settings SET site_address=? WHERE id=1",("https://example.test/journal/",));d.commit();d.close()' $db
  if ($LASTEXITCODE -ne 0) { throw "Could not configure the isolated canonical origin" }
  foreach ($collection in @('posts', 'pages')) {
    $public = Invoke-Request -Path "/$collection/seo-story?lang=fr&ref=example"
    $head = $public.Body.Split('</head>')[0]
    $canonical = '<link(?=[^>]*rel="canonical")(?=[^>]*href="https://example\.test/journal/' + $collection + '/seo-story")[^>]*>'
    if ([regex]::Matches($head, $canonical).Count -ne 1 -or
        [regex]::Matches($public.Body, 'rel="canonical"').Count -ne 1) {
      throw "Published $collection canonical must use the configured address once and omit the query"
    }
    $private = Invoke-Request -Path "/$collection/seo-draft" -Status 404
    if ($private.Body -match 'Hidden SEO draft|Private SEO excerpt|rel="canonical"') {
      throw "Draft $collection leaked public metadata"
    }
  }
  $admin = Invoke-Request -Path '/admin' -Headers @{ Cookie = $cookie }
  if ($admin.Body -match 'rel="canonical"|name="description"') {
    throw "Public SEO metadata leaked into the admin layout"
  }
} finally {
  & $python.Source -c 'import sqlite3,sys;d=sqlite3.connect(sys.argv[1]);d.execute("UPDATE legura_general_settings SET site_address=? WHERE id=1",("",));d.execute("DELETE FROM legura_posts WHERE id IN (?,?)",("seo-story","seo-draft"));d.execute("DELETE FROM legura_pages WHERE id IN (?,?)",("seo-story","seo-draft"));d.commit();d.close()' $db
  if ($LASTEXITCODE -ne 0) { throw "Could not clear isolated public metadata fixtures" }
}
Write-Host 'Legura public metadata HTTP passed: escaped title and description, canonical host/path, draft and admin boundaries.'
