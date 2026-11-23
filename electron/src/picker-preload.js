// The screen-share picker's window: the list of screens and windows, and the choice.

'use strict';

const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('picker', {
	load: () => ipcRenderer.invoke('picker:sources'),
	choose: (id) => ipcRenderer.send('picker:choose', id)
});
