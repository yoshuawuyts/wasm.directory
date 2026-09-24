/* Run in the head before rendering the page to prevent a wrong-theme flash. */
(function() {
  var root = document.documentElement;
  var stored = localStorage.getItem('ds-theme');
  var explicit = stored === 'dark' || stored === 'light' ? stored : null;
  var mode = explicit || (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
  if (explicit) root.setAttribute('data-theme', explicit);
  root.style.background = mode === 'dark' ? '#1C1C20' : '#F4F4F5';
  root.style.colorScheme = mode;
})();
