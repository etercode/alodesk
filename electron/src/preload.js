// What the site may ask of the app: window.aloDesktop, the same functions as
// the Windows app's (alo-sk's src/lib/state/native.js uses either). Only
// alo.devl0pr.com loads in the window, and main.js checks the sender again.

'use strict';

const { contextBridge, ipcRenderer } = require('electron');

/** @param {string} channel @param {(value: unknown) => void} handler */
function listen(channel, handler) {
	const listener = (_event, value) => handler(value);
	ipcRenderer.on(channel, listener);
	return () => ipcRenderer.removeListener(channel, listener);
}

contextBridge.exposeInMainWorld('aloDesktop', {
	platform: process.platform,
	/** The push-to-talk key (a KeyboardEvent.code), or '' for none. */
	setPushKey: (code) => ipcRenderer.invoke('alo:set-push-key', String(code ?? '')),
	/** The mute and deafen hotkeys ("Control+Shift+KeyM"), '' for none. */
	setHotkeys: (mute, deafen) => ipcRenderer.invoke('alo:set-hotkeys', String(mute ?? ''), String(deafen ?? '')),
	/** The push-to-talk key down (true) or up (false) in another app. Returns a stop function. */
	onPush: (handler) => listen('alo:ptt', (value) => handler(value === true)),
	/** "mute" or "deafen", pushed in another app. Returns a stop function. */
	onHotkey: (handler) =>
		listen('alo:hotkey', (value) => {
			if (value === 'mute' || value === 'deafen') handler(value);
		})
});
