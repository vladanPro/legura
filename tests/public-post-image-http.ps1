# Only the disposable media fixture owns these posts and objects.
$publicImageId = $first[0]
$publicImageUrl = "/api/post-images/image-fixture/$publicImageId"
$updateRoute = [Uri]::EscapeDataString("/admin/posts/$postImagePostId/edit")
$updatePostPath = "/__axonyx/action?path=$updateRoute&name=UpdatePost"
function Set-PublicImagePost([string]$Status, [string]$Slug = 'image-fixture') {
  Invoke-Request -Path $updatePostPath -Method POST -Body "id=$postImagePostId&title=Image+fixture&slug=$Slug&body=Unchanged+body&status=$Status" -Headers $sessionProof -Status 303 | Out-Null
}
function Assert-PublicImage([string]$Path, [int]$Status, [string]$Id = '') {
  $request = [Net.Http.HttpRequestMessage]::new([Net.Http.HttpMethod]::Get, "$baseUrl$Path")
  $request.Headers.TryAddWithoutValidation('If-None-Match', '*') | Out-Null
  try {
    $response = $client.SendAsync($request).GetAwaiter().GetResult()
    try {
      if ([int]$response.StatusCode -ne $Status) { throw "Public image expected $Status, got $([int]$response.StatusCode): $Path" }
      if (!$response.Headers.CacheControl.NoStore -or !$response.Headers.CacheControl.Private -or $response.Headers.Contains('Set-Cookie')) { throw 'Public image cache/session policy failed' }
      if (($response.Headers.GetValues('X-Content-Type-Options') -join '') -ne 'nosniff' -or ($response.Headers.GetValues('Cross-Origin-Resource-Policy') -join '') -ne 'same-origin') { throw 'Public image headers failed' }
      $bytes = $response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
      if ($Status -eq 200) {
        if ($response.Content.Headers.ContentType.MediaType -ne 'image/png' -or [Convert]::ToBase64String($bytes) -ne [Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $mediaRoot $Id)))) { throw 'Public image bytes changed' }
      } else {
        $expected = if ($Status -eq 404) { 'not found' } else { 'binary response unavailable' }
        if ([Text.Encoding]::UTF8.GetString($bytes) -ne $expected) { throw 'Public image leaked internal details' }
      }
    } finally { $response.Dispose() }
  } finally { $request.Dispose() }
}
Assert-PublicImage $publicImageUrl 404
Set-PublicImagePost 'published'
Assert-PublicImage $publicImageUrl 200 $publicImageId
Assert-PrivateMediaRead $publicImageId '' 403
Assert-PublicImage "/api/post-images/image-fixture/$($second[0])" 404
Assert-PublicImage "/api/post-images/missing/$publicImageId" 404
$publicPage = Invoke-Request -Path '/posts/image-fixture'
if (!$publicPage.Body.Contains($publicImageUrl) -or !$publicPage.Body.Contains('data-post-featured-image="true"')) { throw 'Published post did not render its image' }
Invoke-Request -Path $postImageSetPath -Method POST -Body "id=$postImagePostId&media_id=$publicImageId&alt=$([Uri]::EscapeDataString($literalAlt))" -Headers $sessionProof -Status 303 | Out-Null
$publicPage = Invoke-Request -Path '/posts/image-fixture'
if ($publicPage.Body.Contains('alt="<script>') -or !$publicPage.Body.Contains('&lt;script&gt;')) { throw 'Published image alt was not escaped' }
if ($MediaBrowser) {
  @{baseUrl=$baseUrl; imageUrl=$publicImageUrl; alt=$literalAlt} | ConvertTo-Json -Compress | & node (Join-Path $root 'tests/public-post-image-browser.mjs')
  if ($LASTEXITCODE -ne 0) { throw 'Public image browser proof failed' }
}
$objectPath = Join-Path $mediaRoot $publicImageId
$savedBytes = [IO.File]::ReadAllBytes($objectPath)
try {
  [IO.File]::WriteAllBytes($objectPath, [byte[]]@(0,1,2))
  Assert-PublicImage $publicImageUrl 500
  $fallback = Invoke-Request -Path '/posts/image-fixture'
  if (!$fallback.Body.Contains('data-post-image-unavailable="true"') -or $fallback.Body.Contains('data-post-featured-image="true"')) { throw 'Unavailable image did not render fallback' }
} finally { [IO.File]::WriteAllBytes($objectPath, $savedBytes) }
Set-PublicImagePost 'draft'
Assert-PublicImage $publicImageUrl 404
Set-PublicImagePost 'published' 'renamed-image-fixture'
Assert-PublicImage $publicImageUrl 404
Assert-PublicImage "/api/post-images/renamed-image-fixture/$publicImageId" 200 $publicImageId
Set-PublicImagePost 'published'
Invoke-Request -Path $postImageSetPath -Method POST -Body "id=$postImagePostId&media_id=$($second[0])&alt=Replacement" -Headers $sessionProof -Status 303 | Out-Null
Assert-PublicImage $publicImageUrl 404
Assert-PublicImage "/api/post-images/image-fixture/$($second[0])" 200 $second[0]
Invoke-Request -Path $postImageRemovePath -Method POST -Body "id=$postImagePostId" -Headers $sessionProof -Status 303 | Out-Null
Assert-PublicImage "/api/post-images/image-fixture/$($second[0])" 404
$empty = Invoke-Request -Path '/posts/image-fixture'
if ($empty.Body.Contains('data-post-featured-image="true"') -or $empty.Body.Contains('data-post-image-unavailable="true"')) { throw 'Unselected image should leave no placeholder' }
# Restore the exact association/content expected by archive/recovery acceptance.
Set-PublicImagePost 'draft'
Invoke-Request -Path $postImageSetPath -Method POST -Body $postImageForm -Headers $sessionProof -Status 303 | Out-Null
Assert-PostImage $first[0] 'Fixture image'
Write-Host 'Public post images passed: anonymous exact bytes, private isolation, escaped markup, fallback, unpublish/rename/replace/detach revocation.'
