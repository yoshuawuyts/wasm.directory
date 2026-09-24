const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const vm = require('node:vm');

const init = readFileSync(path.join(__dirname, 'init.js'), 'utf8');
const controls = readFileSync(path.join(__dirname, 'controls.js'), 'utf8');

function element(attributes = {}) {
  return {
    attributes: new Map(Object.entries(attributes)),
    style: {},
    events: new Map(),
    getAttribute(name) { return this.attributes.get(name) ?? null; },
    setAttribute(name, value) { this.attributes.set(name, value); },
    addEventListener(name, callback) { this.events.set(name, callback); },
  };
}

function button() {
  const trigger = element({
    type: 'button',
    'aria-label': 'Dark mode',
    'aria-pressed': 'false',
  });
  const icons = {
    '.theme-icon-light': element({ 'aria-hidden': 'true' }),
    '.theme-icon-dark': element({ 'aria-hidden': 'true' }),
  };
  trigger.querySelector = selector => {
    assert.ok(icons[selector], `unknown icon selector: ${selector}`);
    return icons[selector];
  };
  trigger.click = () => trigger.events.get('click')();
  return trigger;
}

function page({ system, stored = null, count = 4 }) {
  const storage = new Map(stored === null ? [] : [['ds-theme', stored]]);
  const root = element();
  const triggers = Array.from({ length: count }, button);
  const mq = element();
  mq.matches = system === 'dark';
  const context = vm.createContext({
    document: {
      documentElement: root,
      querySelectorAll(selector) {
        assert.equal(selector, '.theme-toggle');
        return triggers;
      },
    },
    window: {
      matchMedia(query) {
        assert.equal(query, '(prefers-color-scheme: dark)');
        return mq;
      },
    },
    localStorage: {
      getItem(key) { return storage.get(key) ?? null; },
      setItem(key, value) { storage.set(key, value); },
    },
  });
  vm.runInContext(init, context);
  return {
    root,
    triggers,
    storage,
    startControls() { vm.runInContext(controls, context); },
    changeSystem(mode) {
      mq.matches = mode === 'dark';
      mq.events.get('change')();
    },
  };
}

function assertScheme(rendered, mode, explicit) {
  assert.equal(rendered.root.getAttribute('data-theme'), explicit);
  assert.equal(rendered.root.style.background, mode === 'dark' ? '#1C1C20' : '#F4F4F5');
  assert.equal(rendered.root.style.colorScheme, mode);
}

function assertControls(rendered, mode) {
  const next = mode === 'dark' ? 'light' : 'dark';
  for (const trigger of rendered.triggers) {
    assert.equal(trigger.getAttribute('aria-label'), 'Dark mode');
    assert.equal(trigger.getAttribute('aria-pressed'), String(mode === 'dark'));
    assert.equal(trigger.getAttribute('title'), `Switch to ${next} theme`);
    assert.equal(trigger.querySelector(`.theme-icon-${mode}`).style.display, '');
    assert.equal(trigger.querySelector(`.theme-icon-${next}`).style.display, 'none');
    assert.equal(trigger.querySelector(`.theme-icon-${mode}`).getAttribute('aria-hidden'), 'true');
    assert.deepEqual(trigger.style, {});
  }
}

function verifyRepeatedClicks(system, stored) {
  const rendered = page({ system, stored });
  const explicit = stored === 'dark' || stored === 'light' ? stored : null;
  let mode = explicit || system;
  assertScheme(rendered, mode, explicit);
  rendered.startControls();
  assertControls(rendered, mode);
  for (const trigger of rendered.triggers) {
    mode = mode === 'dark' ? 'light' : 'dark';
    trigger.click();
    assertScheme(rendered, mode, mode);
    assertControls(rendered, mode);
    assert.equal(rendered.storage.get('ds-theme'), mode);
    const reloaded = page({ system, stored: rendered.storage.get('ds-theme') });
    assertScheme(reloaded, mode, mode);
    reloaded.startControls();
    assertControls(reloaded, mode);
  }
}

for (const system of ['light', 'dark']) {
  for (const stored of [null, 'invalid', 'light', 'dark']) {
    test(`system ${system}, stored ${stored}: repeated clicks persist and reload`, () => {
      verifyRepeatedClicks(system, stored);
    });
  }

  test(`system ${system}: initialize before controls without writing a preference`, () => {
    for (const stored of [null, '', 'system', 'invalid']) {
      const rendered = page({ system, stored });
      assertScheme(rendered, system, null);
      assert.equal(rendered.storage.get('ds-theme') ?? null, stored);
      rendered.startControls();
      assertControls(rendered, system);
    }
  });

  test(`system ${system}: follow live changes only until a choice is made`, () => {
    const rendered = page({ system });
    rendered.startControls();
    const opposite = system === 'light' ? 'dark' : 'light';
    rendered.changeSystem(opposite);
    assertScheme(rendered, opposite, null);
    assertControls(rendered, opposite);
    assert.equal(rendered.storage.has('ds-theme'), false);
    rendered.triggers[0].click();
    assertScheme(rendered, system, system);
    rendered.changeSystem(system);
    rendered.changeSystem(opposite);
    assertScheme(rendered, system, system);
    assertControls(rendered, system);
    assert.equal(rendered.storage.get('ds-theme'), system);
  });

  test(`system ${system}: honor a saved matching choice after the OS changes`, () => {
    const rendered = page({ system, stored: system });
    rendered.startControls();
    rendered.changeSystem(system === 'light' ? 'dark' : 'light');
    assertScheme(rendered, system, system);
    assertControls(rendered, system);
  });

  test(`system ${system}: pages without controls still track system changes`, () => {
    const rendered = page({ system, count: 0 });
    rendered.startControls();
    const opposite = system === 'light' ? 'dark' : 'light';
    rendered.changeSystem(opposite);
    assertScheme(rendered, opposite, null);
  });
}
