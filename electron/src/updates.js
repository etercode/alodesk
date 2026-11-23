// The app updates itself from the latest GitHub release (latest-linux.yml,
// written by electron-builder), as the Windows app does (src-tauri/src/update.rs):
//
// - AppImage: a newer version found at start is installed at once and alo
//   starts again; nobody is in a call yet. Later ones wait for "Restart to
//   update" in the tray menu, or install when alo quits.
// - .deb: installing needs the computer's password (pkexec), so it never
//   happens by itself: the tray menu offers "Restart to update", with a
//   notification.
//
// electron-updater checks the download's SHA-512 from latest-linux.yml.

'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { app, Notification } = require('electron');
const { autoUpdater } = require('electron-updater');

const EVERY_MS = 6 * 60 * 60 * 1000;
/** A download that ends this soon after the start installs at once (AppImage). */
const AT_START_MS = 30 * 1000;

let pending = null;

/** How this copy was installed: "appimage", "deb", or something else. */
function kind() {
	if (process.env.APPIMAGE) return 'appimage';
	try {
		return fs.readFileSync(path.join(process.resourcesPath, 'package-type'), 'utf8').trim();
	} catch {
		return 'other';
	}
}

/** @param {() => void} changed when an update starts to wait (the tray menu changes) */
function start(changed) {
	if (!app.isPackaged) return;
	const type = kind();
	if (type !== 'appimage' && type !== 'deb') return;

	const startedAt = Date.now();
	autoUpdater.autoDownload = true;
	autoUpdater.autoInstallOnAppQuit = type === 'appimage';
	autoUpdater.on('error', (error) => console.error('update:', error?.message ?? error));
	autoUpdater.on('update-downloaded', (info) => {
		if (type === 'appimage' && Date.now() - startedAt < AT_START_MS) {
			autoUpdater.quitAndInstall(true, true);
			return;
		}
		pending = info.version;
		changed();
		new Notification({
			title: `alo ${info.version} is ready`,
			body:
				type === 'deb'
					? 'Right-click alo\'s tray icon and choose "Restart to update" when you are not in a call. It asks for your password.'
					: 'Right-click alo\'s tray icon and choose "Restart to update" when you are not in a call.'
		}).show();
	});

	const check = () => {
		if (!pending) autoUpdater.checkForUpdates().catch(() => {});
	};
	check();
	setInterval(check, EVERY_MS);
}

/** The version that waits, or null. */
function waiting() {
	return pending;
}

/** "Restart to update". */
function install() {
	if (pending) autoUpdater.quitAndInstall(false, true);
}

module.exports = { start, waiting, install };
