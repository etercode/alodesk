// The alo desktop app for Linux (and, later, macOS): https://alo.devl0pr.com
// in an Electron window, so voice and video run on Chrome's engine. Windows
// has its own, smaller app in ../src-tauri. Both add the same things to the
// site, and the site talks to either through alo-sk's src/lib/state/native.js:
//
// - push-to-talk and the mute and deafen keys while another app has focus (keys.js)
// - the microphone, camera and notifications for alo without asking
// - a picker for screen share
// - a tray icon: close hides to it; start with the computer
// - alo://invite/CODE and alo://open links, and one copy at a time
// - every other link, the feelm sign-in too, in the default browser
// - updates of itself (updates.js)

'use strict';

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { app, BrowserWindow, Menu, Tray, desktopCapturer, ipcMain, nativeImage, screen, session, shell } = require('electron');
const { Keys } = require('./keys');
const updates = require('./updates');

// A development run may show a local alo (ALO_ORIGIN=http://localhost:5178).
const ORIGIN = new URL((!app.isPackaged && process.env.ALO_ORIGIN) || 'https://alo.devl0pr.com').origin;
const MINIMIZED = '--minimized';
const ICON = path.join(__dirname, '..', 'assets', 'icon.png');
const TRAY_ICON = path.join(__dirname, '..', 'assets', 'tray.png');

/** A page of alo itself. @param {string} url */
function isAlo(url) {
	try {
		return new URL(url).origin === ORIGIN;
	} catch {
		return false;
	}
}

// Timers keep their pace and the page its priority while the window is in the tray or behind a
// game, so voice and push-to-talk never lag. The page itself does count as hidden then: it stops
// drawing and pauses the videos, and a game keeps the graphics card.
app.commandLine.appendSwitch('disable-renderer-backgrounding');
app.commandLine.appendSwitch('disable-background-timer-throttling');
app.commandLine.appendSwitch('autoplay-policy', 'no-user-gesture-required');

// The app's own settings, needed before the window exists (the site cannot keep them): for now
// whether it draws with the graphics card. A change takes effect at the next start.
const prefsFile = () => path.join(app.getPath('userData'), 'prefs.json');

function readPrefs() {
	try {
		const saved = JSON.parse(fs.readFileSync(prefsFile(), 'utf8'));
		return { hardwareAcceleration: saved.hardwareAcceleration !== false };
	} catch {
		return { hardwareAcceleration: true };
	}
}

const running = readPrefs();
if (!running.hardwareAcceleration) app.disableHardwareAcceleration();
// A development run opens the DevTools protocol for testing; a packaged app does not.
if (!app.isPackaged) app.commandLine.appendSwitch('remote-debugging-port', process.env.ALO_DEBUG_PORT || '9230');

/** @type {BrowserWindow | null} */
let win = null;
/** @type {Tray | null} */
let tray = null;
let quitting = false;

if (!app.requestSingleInstanceLock()) {
	// A second start (or a clicked alo:// link) goes to the running copy.
	app.quit();
} else {
	app.on('second-instance', (_event, argv) => {
		show();
		openLinks(argv);
	});
	app.on('before-quit', () => (quitting = true));
	app.whenReady().then(ready);
}

function ready() {
	if (app.isPackaged && process.platform !== 'linux') app.setAsDefaultProtocolClient('alo');
	setUpSession();
	createWindow(!process.argv.includes(MINIMIZED));
	createTray();
	const keys = new Keys(
		(channel, value) => win?.webContents.send(channel, value),
		() => !!win && win.isFocused()
	);
	ipcMain.handle('alo:set-push-key', (event, code) => fromAlo(event) && keys.setPush(String(code)));
	ipcMain.handle('alo:set-hotkeys', (event, mute, deafen) => fromAlo(event) && keys.setHotkeys(String(mute), String(deafen)));
	ipcMain.handle('alo:get-hardware-acceleration', (event) =>
		fromAlo(event) ? { on: readPrefs().hardwareAcceleration, running: running.hardwareAcceleration } : null
	);
	ipcMain.handle('alo:set-hardware-acceleration', (event, on) => {
		if (!fromAlo(event)) return false;
		fs.mkdirSync(path.dirname(prefsFile()), { recursive: true });
		fs.writeFileSync(prefsFile(), JSON.stringify({ ...readPrefs(), hardwareAcceleration: on === true }));
		return true;
	});
	ipcMain.handle('alo:restart', (event) => {
		if (!fromAlo(event)) return;
		// An AppImage starts again from its own file, not from the folder it unpacked to.
		app.relaunch(process.env.APPIMAGE ? { execPath: process.env.APPIMAGE, args: [] } : undefined);
		app.quit();
	});
	updates.start(refreshTray);
	openLinks(process.argv);
}

/** The page in alo's window, not some other frame. @param {Electron.IpcMainInvokeEvent} event */
function fromAlo(event) {
	return !!win && event.sender === win.webContents && isAlo(event.senderFrame?.url ?? '');
}

// The microphone, camera, notifications and screen share for alo itself, without asking, as in Discord.
const ALLOWED = new Set(['media', 'notifications', 'fullscreen', 'clipboard-sanitized-write', 'display-capture', 'speaker-selection']);

function setUpSession() {
	const ses = session.defaultSession;
	ses.setPermissionRequestHandler((contents, permission, callback, details) =>
		callback(ALLOWED.has(permission) && isAlo(details.requestingUrl || contents.getURL()))
	);
	ses.setPermissionCheckHandler((_contents, permission, origin) => ALLOWED.has(permission) && isAlo(origin));
	ses.setDisplayMediaRequestHandler(
		async (request, callback) => {
			try {
				if (!isAlo(request.frame?.url ?? request.securityOrigin ?? '')) return callback({});
				// Under Wayland the system's own dialog picks, and one source comes back.
				const sources = await desktopCapturer.getSources({ types: ['screen', 'window'], thumbnailSize: { width: 320, height: 180 } });
				const source = sources.length === 1 ? sources[0] : await pick(sources);
				callback(source ? { video: source } : {});
			} catch (error) {
				console.error('screen share:', error);
				callback({});
			}
		},
		{ useSystemPicker: true }
	);
}

/** Our own picker for screen share (X11): a small window with a picture of each screen and window. */
function pick(sources) {
	return new Promise((resolve) => {
		const picker = new BrowserWindow({
			parent: win ?? undefined,
			modal: true,
			width: 780,
			height: 560,
			title: 'Share your screen',
			icon: ICON,
			autoHideMenuBar: true,
			backgroundColor: '#1e1f22',
			webPreferences: { preload: path.join(__dirname, 'picker-preload.js'), contextIsolation: true, sandbox: true }
		});
		picker.removeMenu();
		const list = sources.map((source) => ({
			id: source.id,
			name: source.name,
			screen: source.id.startsWith('screen:'),
			thumbnail: source.thumbnail.toDataURL()
		}));
		let done = false;
		const finish = (id) => {
			if (done) return;
			done = true;
			ipcMain.removeHandler('picker:sources');
			ipcMain.removeAllListeners('picker:choose');
			resolve(sources.find((source) => source.id === id) ?? null);
			if (!picker.isDestroyed()) picker.close();
		};
		ipcMain.handle('picker:sources', (event) => (event.sender === picker.webContents ? { sources: list, locale: app.getLocale() } : null));
		ipcMain.on('picker:choose', (event, id) => {
			if (event.sender === picker.webContents) finish(typeof id === 'string' ? id : null);
		});
		picker.on('closed', () => finish(null));
		picker.loadFile(path.join(__dirname, 'picker.html'));
	});
}

function createWindow(visible) {
	const saved = readWindowState();
	win = new BrowserWindow({
		width: saved?.width ?? 1280,
		height: saved?.height ?? 800,
		x: saved?.x,
		y: saved?.y,
		minWidth: 940,
		minHeight: 560,
		show: false,
		title: 'alo',
		icon: ICON,
		autoHideMenuBar: true,
		backgroundColor: '#1e1f22',
		webPreferences: {
			preload: path.join(__dirname, 'preload.js'),
			contextIsolation: true,
			sandbox: true,
			// Hidden (tray, minimized) the page counts as hidden: no drawing, no videos. The
			// switches above keep its timers on time.
			backgroundThrottling: true,
			spellcheck: true
		}
	});
	win.removeMenu();
	if (saved?.maximized) win.maximize();
	if (visible) win.once('ready-to-show', () => win?.show());

	const contents = win.webContents;
	// alo opens in the window, every other link in the default browser (the feelm sign-in too:
	// the browser leaves the code at alo-api and the page in the window takes it).
	const leave = (event, url) => {
		if (isAlo(url)) return;
		event.preventDefault();
		openOutside(url);
	};
	contents.on('will-navigate', leave);
	contents.on('will-redirect', leave);
	contents.setWindowOpenHandler(({ url }) => {
		openOutside(url);
		return { action: 'deny' };
	});

	// The close button hides to the tray; "Quit alo" in the tray ends alo.
	win.on('close', (event) => {
		saveWindowState();
		if (!quitting) {
			event.preventDefault();
			win?.hide();
		}
	});
	let timer = null;
	const later = () => {
		clearTimeout(timer);
		timer = setTimeout(saveWindowState, 500);
	};
	win.on('resize', later);
	win.on('move', later);

	win.loadURL(ORIGIN);
}

/** @param {string} url */
function openOutside(url) {
	if (/^(https?|mailto):/i.test(url)) shell.openExternal(url).catch(() => {});
}

function show() {
	if (!win) return;
	if (win.isMinimized()) win.restore();
	win.show();
	win.focus();
}

/** alo://open brings the window to the front; alo://invite/CODE opens that invite. @param {string[]} argv */
function openLinks(argv) {
	for (const arg of argv) {
		if (!arg.startsWith('alo://')) continue;
		let url;
		try {
			url = new URL(arg);
		} catch {
			continue;
		}
		const code = url.pathname.replace(/^\/+|\/+$/g, '');
		if (url.host === 'open') {
			show();
		} else if (url.host === 'invite' && /^[A-Za-z0-9._~-]{1,64}$/.test(code)) {
			win?.loadURL(`${ORIGIN}/invite/${code}`);
			show();
		}
	}
}

// Size and place, kept between starts.
const stateFile = () => path.join(app.getPath('userData'), 'window.json');

function readWindowState() {
	try {
		const state = JSON.parse(fs.readFileSync(stateFile(), 'utf8'));
		// A screen that is gone: the system places the window.
		const visible = screen.getAllDisplays().some(({ workArea: a }) => state.x < a.x + a.width && state.x + state.width > a.x && state.y < a.y + a.height && state.y + state.height > a.y);
		if (!visible) {
			delete state.x;
			delete state.y;
		}
		return state;
	} catch {
		return null;
	}
}

function saveWindowState() {
	if (!win || win.isDestroyed()) return;
	try {
		fs.writeFileSync(stateFile(), JSON.stringify({ ...win.getNormalBounds(), maximized: win.isMaximized() }));
	} catch {
		// A full disk: the next start uses the default size.
	}
}

// Start with the computer: an autostart entry on Linux, a login item on macOS. Off at first.
const autostartFile = path.join(process.env.XDG_CONFIG_HOME || path.join(os.homedir(), '.config'), 'autostart', 'alo.desktop');

function autostartOn() {
	if (process.platform === 'linux') return fs.existsSync(autostartFile);
	return app.getLoginItemSettings().openAtLogin;
}

function setAutostart(on) {
	if (process.platform !== 'linux') {
		app.setLoginItemSettings({ openAtLogin: on, args: [MINIMIZED] });
		return;
	}
	if (!on) {
		fs.rmSync(autostartFile, { force: true });
		return;
	}
	// The AppImage file itself, not the folder it unpacks to for each start.
	const exe = process.env.APPIMAGE || process.execPath;
	fs.mkdirSync(path.dirname(autostartFile), { recursive: true });
	fs.writeFileSync(
		autostartFile,
		['[Desktop Entry]', 'Type=Application', 'Name=alo', `Exec="${exe.replace(/(["`$\\])/g, '\\$1')}" ${MINIMIZED}`, 'Icon=alo', 'X-GNOME-Autostart-enabled=true', ''].join('\n')
	);
}

function createTray() {
	tray = new Tray(nativeImage.createFromPath(TRAY_ICON));
	tray.setToolTip('alo');
	tray.on('click', show);
	refreshTray();
}

function refreshTray() {
	if (!tray) return;
	const waiting = updates.waiting();
	tray.setContextMenu(
		Menu.buildFromTemplate([
			waiting
				? { label: `Restart to update to ${waiting}`, click: () => updates.install() }
				: { label: `alo ${app.getVersion()}`, enabled: false },
			{ type: 'separator' },
			{ label: 'Open alo', click: show },
			// Back to alo's start, whatever the window shows.
			{ label: 'Reload', click: () => (win?.loadURL(ORIGIN), show()) },
			{
				label: process.platform === 'darwin' ? 'Open at login' : 'Start with the computer',
				type: 'checkbox',
				checked: autostartOn(),
				click: (item) => {
					try {
						setAutostart(item.checked);
					} catch (error) {
						console.error('autostart:', error);
					}
					refreshTray();
				}
			},
			{ type: 'separator' },
			{ label: 'Quit alo', click: () => app.quit() }
		])
	);
}
