'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { UiohookKey } = require('uiohook-napi');
const { keycode, mouseButton, parseCombo } = require('../src/keys');

test('key codes from KeyboardEvent.code', () => {
	assert.equal(keycode('KeyV'), UiohookKey.V);
	assert.equal(keycode('Digit1'), UiohookKey['1']);
	assert.equal(keycode('F13'), UiohookKey.F13);
	assert.equal(keycode('ShiftLeft'), UiohookKey.Shift);
	assert.equal(keycode('ControlRight'), UiohookKey.CtrlRight);
	assert.equal(keycode('Space'), UiohookKey.Space);
	assert.equal(keycode('Numpad5'), UiohookKey.Numpad5);
	assert.equal(keycode('Nope'), null);
	assert.equal(keycode('constructor'), null);
});

test('mouse buttons for push-to-talk', () => {
	assert.equal(mouseButton('Mouse3'), 3);
	assert.equal(mouseButton('Mouse4'), 4);
	assert.equal(mouseButton('Mouse5'), 5);
	// Left and right click: never.
	assert.equal(mouseButton('Mouse1'), null);
	assert.equal(mouseButton('KeyV'), null);
	assert.equal(mouseButton('constructor'), null);
});

test('hotkeys with their modifiers', () => {
	assert.deepEqual(parseCombo('Control+Shift+KeyM'), { keycode: UiohookKey.M, control: true, alt: false, shift: true, meta: false });
	assert.deepEqual(parseCombo('F9'), { keycode: UiohookKey.F9, control: false, alt: false, shift: false, meta: false });
	assert.equal(parseCombo('Hyper+F9'), null);
	assert.equal(parseCombo('Control+Nope'), null);
	assert.equal(parseCombo(''), null);
});
