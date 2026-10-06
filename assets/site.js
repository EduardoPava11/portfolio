(function () {
  var reel = document.getElementById('reel');
  var slides = Array.prototype.slice.call(reel.querySelectorAll('.slide'));
  var pictures = slides.filter(function (s) { return s.hasAttribute('data-n'); });
  var cells = Array.prototype.slice.call(document.querySelectorAll('.strip a'));
  var strip = document.querySelector('.strip');
  var navLinks = Array.prototype.slice.call(document.querySelectorAll('.top nav a'));

  // Land on the screen named in the URL, instantly, before anything observes the reel.
  var startSlide = slides[0];
  if (location.hash) {
    var target = document.getElementById(location.hash.slice(1));
    if (target && slides.indexOf(target) >= 0) startSlide = target;
  }
  reel.style.scrollBehavior = 'auto';
  reel.scrollTop = startSlide.offsetTop;
  reel.style.scrollBehavior = '';

  // Which screen is up: light its strip cell, mark the nav word, keep the hash honest.
  var current = slides.indexOf(startSlide);
  function setCurrent(i) {
    current = i;
    var s = slides[i];
    s.classList.add('in');
    var pi = pictures.indexOf(s);
    cells.forEach(function (c, j) { c.classList.toggle('current', j === pi); });
    strip.classList.toggle('away', pi < 0);
    var section = s.hasAttribute('data-n') ? 'p01' : s.id;
    navLinks.forEach(function (a) { a.classList.toggle('current', a.getAttribute('href') === '#' + section); });
    if (history.replaceState) history.replaceState(null, '', '#' + s.id);
  }
  if ('IntersectionObserver' in window) {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) {
        if (e.isIntersecting && e.intersectionRatio >= 0.25) e.target.classList.add('in');
        if (e.isIntersecting && e.intersectionRatio >= 0.6) setCurrent(slides.indexOf(e.target));
      });
    }, { root: reel, threshold: [0.25, 0.6] });
    slides.forEach(function (s) { io.observe(s); });
  }

  // Arrow keys and space step through; swipe and wheel are native.
  function go(i) {
    if (i < 0 || i >= slides.length) return;
    slides[i].scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
  document.addEventListener('keydown', function (e) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowRight' || e.key === ' ' || e.key === 'PageDown') { e.preventDefault(); go(current + 1); }
    if (e.key === 'ArrowUp' || e.key === 'ArrowLeft' || e.key === 'PageUp') { e.preventDefault(); go(current - 1); }
    if (e.key === 'Home') { e.preventDefault(); go(0); }
    if (e.key === 'End') { e.preventDefault(); go(slides.length - 1); }
  });
  cells.forEach(function (c, i) {
    c.addEventListener('click', function (e) { e.preventDefault(); go(slides.indexOf(pictures[i])); });
  });
  document.querySelectorAll('a[href^="#"]').forEach(function (a) {
    a.addEventListener('click', function (e) {
      var t = document.getElementById(a.getAttribute('href').slice(1));
      if (t && slides.indexOf(t) >= 0) { e.preventDefault(); go(slides.indexOf(t)); }
    });
  });

  // Copy: the paragraphs as plain text, blank line between them.
  document.querySelectorAll('button.copy[data-copy]').forEach(function (btn) {
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

  setCurrent(current);
})();
