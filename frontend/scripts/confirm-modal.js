// The app asks before destructive actions in its own modal (ConfirmDialog),
// not with window.confirm, so Playwright's 'dialog' event never fires for it.
// These helpers answer that modal instead.

// Runs in the page: answers every confirmation modal as it appears.
function install(accept) {
  if (window.__nabimanAutoConfirm) return;
  window.__nabimanAutoConfirm = true;
  const answer = () => {
    const buttons = document.querySelectorAll('.confirm-dialog .confirm-actions button');
    const button = accept ? buttons[buttons.length - 1] : buttons[0];
    if (button && !button.dataset.smokeAnswered) {
      button.dataset.smokeAnswered = '1';
      button.click();
    }
  };
  new MutationObserver(answer).observe(document, { childList: true, subtree: true });
  answer();
}

/** Accepts (or declines) every confirmation, on this page and after navigations,
 *  as `page.on('dialog', d => d.accept())` did for window.confirm. */
exports.autoConfirm = async (page, accept = true) => {
  await page.addInitScript(install, accept);
  await page.evaluate(install, accept).catch(() => {});
};

/** Answers the confirmation that is open now (or about to open). */
exports.answerConfirm = async (page, accept) => {
  const box = page.getByRole('alertdialog');
  await box.waitFor();
  await box.locator('.confirm-actions button').nth(accept ? 1 : 0).click();
  await box.waitFor({ state: 'detached' });
};
