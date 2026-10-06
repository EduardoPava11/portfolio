// Copy buttons: copy the plain text of a section (paragraphs separated by blank lines).
document.querySelectorAll('button.copy[data-copy]').forEach(function (btn) {
  btn.addEventListener('click', async function () {
    var box = document.getElementById(btn.dataset.copy);
    var text = Array.from(box.querySelectorAll('p')).map(function (p) { return p.textContent.trim(); }).join('\n\n');
    var ok = false;
    try {
      await navigator.clipboard.writeText(text);
      ok = true;
    } catch (e) {
      // Older browsers and non secure contexts: select into a hidden textarea.
      var ta = document.createElement('textarea');
      ta.value = text;
      ta.setAttribute('readonly', '');
      ta.style.position = 'fixed';
      ta.style.opacity = '0';
      document.body.appendChild(ta);
      ta.select();
      try { ok = document.execCommand('copy'); } catch (e2) { ok = false; }
      document.body.removeChild(ta);
    }
    var label = btn.textContent;
    btn.textContent = ok ? 'Copied' : 'Select and copy';
    btn.classList.toggle('done', ok);
    setTimeout(function () { btn.textContent = label; btn.classList.remove('done'); }, 1800);
  });
});

// Full view of a photograph in a dialog, with left/right keys to move through the body.
(function () {
  var dialog = document.getElementById('view');
  if (!dialog || !dialog.showModal) return;
  var img = dialog.querySelector('img');
  var cap = dialog.querySelector('figcaption');
  var figures = Array.from(document.querySelectorAll('figure.photo'));
  var current = -1;

  function show(i) {
    if (i < 0 || i >= figures.length) return;
    current = i;
    var f = figures[i];
    img.src = f.dataset.large;
    img.alt = f.dataset.caption;
    cap.textContent = f.dataset.caption;
    if (!dialog.open) dialog.showModal();
  }
  figures.forEach(function (f, i) {
    f.querySelector('a').addEventListener('click', function (e) { e.preventDefault(); show(i); });
  });
  dialog.querySelector('.close').addEventListener('click', function () { dialog.close(); });
  dialog.addEventListener('click', function (e) { if (e.target === dialog || e.target === img.parentNode) dialog.close(); });
  document.addEventListener('keydown', function (e) {
    if (!dialog.open) return;
    if (e.key === 'ArrowRight') show(current + 1);
    if (e.key === 'ArrowLeft') show(current - 1);
  });
})();
