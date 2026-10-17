// Arm before submitting. waitForURL alone can resolve immediately when a save
// redirects back to the current editor URL, before the new document commits.
export async function waitForDocumentNavigation(page, destination) {
  await page.waitForEvent('framenavigated', {
    predicate: frame => frame === page.mainFrame() && frame.url() === destination,
  });
  await page.waitForLoadState('load');
}
