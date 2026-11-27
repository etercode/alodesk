// Global push-to-talk and the mute and deafen hotkeys, while another app has
// focus (as ../../src-tauri/src/ptt.rs does on Windows).
//
// uiohook (libuiohook) hears every key press of the X11 session, without
// taking the key away from the app in front. Under Wayland an app may not
// hear other apps' keys at all; then start() fails and the keys work only
// while alo's window has focus, as in a browser. On macOS it needs the
// "Input Monitoring" permission.
//
// The page names a key as KeyboardEvent.code ("KeyV", "F13", "ShiftLeft"),
// a hotkey with its modifiers before it ("Control+Shift+KeyM", alo-sk's
// util/hotkey.js). uiohook's key codes are physical keys too, so "KeyQ" is
// the same key on a Turkish or Azerbaijani layout.

'use strict';

const { uIOhook, UiohookKey } = require('uiohook-napi');

const SIDES = {
	ControlLeft: 'Ctrl',
	ControlRight: 'CtrlRight',
	AltLeft: 'Alt',
	AltRight: 'AltRight',
	ShiftLeft: 'Shift',
	ShiftRight: 'ShiftRight',
	MetaLeft: 'Meta',
	MetaRight: 'MetaRight',
	OSLeft: 'Meta',
	OSRight: 'MetaRight'
};

/** uiohook's code for a KeyboardEvent.code, or null. @param {string} code */
function keycode(code) {
	let name = SIDES[code] ?? code;
	if (/^Key[A-Z]$/.test(name)) name = name.slice(3);
	else if (/^Digit[0-9]$/.test(name)) name = name.slice(5);
	const value = Object.prototype.hasOwnProperty.call(UiohookKey, name) ? UiohookKey[name] : undefined;
	return typeof value === 'number' ? value : null;
}

// Push-to-talk may be a mouse button too (alo-sk's util/format.js, mouseCode):
// the wheel and the two side buttons, as libuiohook numbers them.
const MOUSE = { Mouse3: 3, Mouse4: 4, Mouse5: 5 };

/** uiohook's button for "Mouse4" and the like, or null. @param {string} code */
function mouseButton(code) {
	return Object.prototype.hasOwnProperty.call(MOUSE, code) ? MOUSE[code] : null;
}

/** "Control+Shift+KeyM" → the key and exactly these modifiers, or null. @param {string} text */
function parseCombo(text) {
	const parts = text.split('+');
	const key = keycode(parts.pop() ?? '');
	if (key === null) return null;
	const combo = { keycode: key, control: false, alt: false, shift: false, meta: false };
	for (const part of parts) {
		if (part === 'Control') combo.control = true;
		else if (part === 'Alt') combo.alt = true;
		else if (part === 'Shift') combo.shift = true;
		else if (part === 'Meta') combo.meta = true;
		else return null;
	}
	return combo;
}

class Keys {
	/**
	 * @param {(channel: string, value: unknown) => void} send to the page
	 * @param {() => boolean} focused alo's window has focus: the page handles its keys itself
	 */
	constructor(send, focused) {
		this.send = send;
		this.focused = focused;
		this.ptt = null;
		this.pttMouse = null;
		this.combos = { mute: null, deafen: null };
		this.down = new Set();
		this.told = false;
		this.running = false;
		this.failed = false;
		uIOhook.on('keydown', (event) => this.#keydown(event));
		uIOhook.on('keyup', (event) => this.#keyup(event));
		uIOhook.on('mousedown', (event) => this.#mouse(event, true));
		uIOhook.on('mouseup', (event) => this.#mouse(event, false));
	}

	/** '' for none. Returns false for a key this app cannot watch. @param {string} code */
	setPush(code) {
		this.pttMouse = code ? mouseButton(code) : null;
		this.ptt = code && this.pttMouse === null ? keycode(code) : null;
		this.#update();
		return !code || ((this.ptt !== null || this.pttMouse !== null) && !this.failed);
	}

	/** '' for none. Returns false when one of them cannot be watched. @param {string} mute @param {string} deafen */
	setHotkeys(mute, deafen) {
		this.combos = { mute: mute ? parseCombo(mute) : null, deafen: deafen ? parseCombo(deafen) : null };
		this.#update();
		return (!mute || !!this.combos.mute) && (!deafen || !!this.combos.deafen) && !this.failed;
	}

	#update() {
		const needed = this.ptt !== null || this.pttMouse !== null || !!this.combos.mute || !!this.combos.deafen;
		if (needed && !this.running && !this.failed) {
			try {
				uIOhook.start();
				this.running = true;
			} catch (error) {
				// Wayland without X11, or no permission on macOS.
				this.failed = true;
				console.error('global keys:', error);
			}
		} else if (!needed && this.running) {
			uIOhook.stop();
			this.running = false;
			this.down.clear();
		}
	}

	#keydown(event) {
		// The key repeats while held: only the first press counts.
		if (this.down.has(event.keycode)) return;
		this.down.add(event.keycode);
		if (this.focused()) return;
		if (event.keycode === this.ptt) {
			this.send('alo:ptt', true);
			this.told = true;
		}
		for (const name of /** @type {const} */ (['mute', 'deafen'])) {
			const combo = this.combos[name];
			if (
				combo &&
				event.keycode === combo.keycode &&
				event.ctrlKey === combo.control &&
				event.altKey === combo.alt &&
				event.shiftKey === combo.shift &&
				event.metaKey === combo.meta
			) {
				this.send('alo:hotkey', name);
			}
		}
	}

	/** The push-to-talk mouse button, down or up, in another app. */
	#mouse(event, down) {
		if (this.pttMouse === null || event.button !== this.pttMouse) return;
		if (down) {
			if (this.focused()) return;
			this.send('alo:ptt', true);
			this.told = true;
		} else if (this.told) {
			this.send('alo:ptt', false);
			this.told = false;
		}
	}

	#keyup(event) {
		this.down.delete(event.keycode);
		if (event.keycode === this.ptt && this.told) {
			this.send('alo:ptt', false);
			this.told = false;
		}
	}
}

module.exports = { Keys, keycode, mouseButton, parseCombo };
