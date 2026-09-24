(function() {
  var triggers = Array.prototype.slice.call(document.querySelectorAll('.theme-toggle'));
  var root = document.documentElement;
  var mq = window.matchMedia('(prefers-color-scheme: dark)');
  var explicit = root.getAttribute('data-theme');

  function effectiveMode() {
    return explicit || (mq.matches ? 'dark' : 'light');
  }

  function updateControls() {
    var mode = effectiveMode();
    var next = mode === 'dark' ? 'light' : 'dark';
    root.style.background = mode === 'dark' ? '#1C1C20' : '#F4F4F5';
    root.style.colorScheme = mode;
    triggers.forEach(function(trigger) {
      trigger.setAttribute('aria-pressed', mode === 'dark' ? 'true' : 'false');
      trigger.setAttribute('title', 'Switch to ' + next + ' theme');
      trigger.querySelector('.theme-icon-light').style.display = mode === 'light' ? '' : 'none';
      trigger.querySelector('.theme-icon-dark').style.display = mode === 'dark' ? '' : 'none';
    });
  }

  updateControls();
  triggers.forEach(function(trigger) {
    trigger.addEventListener('click', function() {
      explicit = effectiveMode() === 'dark' ? 'light' : 'dark';
      root.setAttribute('data-theme', explicit);
      localStorage.setItem('ds-theme', explicit);
      updateControls();
    });
  });

  mq.addEventListener('change', function() {
    if (!explicit) updateControls();
  });
})();
