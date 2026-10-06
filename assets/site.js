(function () {
  var reel = document.getElementById('reel');
  var slides = Array.prototype.slice.call(document.querySelectorAll('.slide[data-n]'));
  var cells = Array.prototype.slice.call(document.querySelectorAll('.strip a'));
  var panel = document.getElementById('panel');

  // Which picture is on screen: light its cell in the strip, keep the hash honest.
  var current = 0;
  function setCurrent(i) {
    current = i;
    cells.forEach(function (c, j) { c.classList.toggle('current', j === i); });
    if (history.replaceState) history.replaceState(null, '', '#p' + String(i + 1).padStart(2, '0'));
  }
  if ('IntersectionObserver' in window) {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting && e.intersectionRatio >= 0.6) setCurrent(slides.indexOf(e.target));
      });
    }, { root: reel, threshold: [0.6] });
    slides.forEach(function (s) { io.observe(s); });
  }

  // Arrow keys and space step through the sequence; swipe and wheel are native.
  function go(i) {
    if (i < 0 || i >= slides.length) return;
    slides[i].scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
  document.addEventListener('keydown', function (e) {
    if (panel.open || e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowRight' || e.key === ' ' || e.key === 'PageDown') { e.preventDefault(); go(current + 1); }
    if (e.key === 'ArrowUp' || e.key === 'ArrowLeft' || e.key === 'PageUp') { e.preventDefault(); go(current - 1); }
    if (e.key === 'Home') { e.preventDefault(); go(0); }
    if (e.key === 'End') { e.preventDefault(); go(slides.length - 1); }
  });
  cells.forEach(function (c, i) {
    c.addEventListener('click', function (e) { e.preventDefault(); go(i); });
  });

  // The words: one panel, two sections, opened from the top edge or the last screen.
  var sections = Array.prototype.slice.call(panel.querySelectorAll('.text'));
  function openPanel(which) {
    sections.forEach(function (s) { s.hidden = s.getAttribute('data-for') !== which; });
    if (!panel.open) panel.showModal();
    var box = panel.querySelector('.text:not([hidden])');
    if (box) box.scrollTop = 0;
  }
  document.querySelectorAll('[data-panel]').forEach(function (b) {
    b.addEventListener('click', function () { openPanel(b.getAttribute('data-panel')); });
  });
  panel.querySelector('.close').addEventListener('click', function () { panel.close(); });
  panel.addEventListener('click', function (e) { if (e.target === panel) panel.close(); });

  // Copy: the paragraphs as plain text, blank line between them.
  panel.querySelectorAll('button.copy[data-copy]').forEach(function (btn) {
    btn.addEventListener('click', function () {
      var box = document.getElementById(btn.getAttribute('data-copy'));
      var text = Array.prototype.map.call(box.querySelectorAll('p'), function (p) { return p.textContent.trim(); }).join('\n\n');
      var done = function (ok) {
        var label = btn.textContent;
        btn.textContent = ok ? 'Copied' : 'Select and copy';
        btn.classList.toggle('done', ok);
        setTimeout(function () { btn.textContent = label; btn.classList.remove('done'); }, 1800);
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text).then(function () { done(true); }, function () { done(fallback(text)); });
      } else {
        done(fallback(text));
      }
    });
  });
  function fallback(text) {
    var ta = document.createElement('textarea');
    ta.value = text; ta.setAttribute('readonly', ''); ta.style.position = 'fixed'; ta.style.opacity = '0';
    document.body.appendChild(ta); ta.select();
    var ok = false;
    try { ok = document.execCommand('copy'); } catch (e) { ok = false; }
    document.body.removeChild(ta);
    return ok;
  }

  // Land on the picture named in the URL.
  var m = /^#p(\d+)$/.exec(location.hash);
  if (m) { var i = parseInt(m[1], 10) - 1; if (slides[i]) { slides[i].scrollIntoView({ block: 'start' }); setCurrent(i); } }
  else setCurrent(0);
})();
