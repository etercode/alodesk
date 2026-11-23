// The screen-share picker: screens or windows, a click picks, a double click or "Share" shares.

'use strict';

const TEXTS = {
	en: { title: 'Share your screen', screens: 'Screens', windows: 'Windows', cancel: 'Cancel', share: 'Share' },
	az: { title: 'Ekranı paylaş', screens: 'Ekranlar', windows: 'Pəncərələr', cancel: 'Ləğv et', share: 'Paylaş' },
	tr: { title: 'Ekranını paylaş', screens: 'Ekranlar', windows: 'Pencereler', cancel: 'İptal', share: 'Paylaş' },
	ru: { title: 'Показать экран', screens: 'Экраны', windows: 'Окна', cancel: 'Отмена', share: 'Показать' }
};

const $ = (id) => document.getElementById(id);
let sources = [];
let showScreens = true;
let chosen = null;

function render() {
	$('screens').setAttribute('aria-pressed', String(showScreens));
	$('windows').setAttribute('aria-pressed', String(!showScreens));
	$('share').disabled = !chosen;
	const list = $('list');
	list.replaceChildren(
		...sources
			.filter((source) => source.screen === showScreens)
			.map((source) => {
				const button = document.createElement('button');
				button.type = 'button';
				button.setAttribute('aria-pressed', String(source.id === chosen));
				const image = document.createElement('img');
				image.src = source.thumbnail;
				image.alt = '';
				const name = document.createElement('span');
				name.textContent = source.name;
				button.append(image, name);
				button.addEventListener('click', () => {
					chosen = source.id;
					render();
				});
				button.addEventListener('dblclick', () => window.picker.choose(source.id));
				return button;
			})
	);
}

window.picker.load().then((data) => {
	if (!data) return;
	const text = TEXTS[String(data.locale).slice(0, 2)] ?? TEXTS.en;
	document.title = text.title;
	$('title').textContent = text.title;
	$('screens').textContent = text.screens;
	$('windows').textContent = text.windows;
	$('cancel').textContent = text.cancel;
	$('share').textContent = text.share;
	sources = data.sources;
	if (!sources.some((source) => source.screen)) showScreens = false;
	render();
});

$('screens').addEventListener('click', () => ((showScreens = true), render()));
$('windows').addEventListener('click', () => ((showScreens = false), render()));
$('cancel').addEventListener('click', () => window.picker.choose(null));
$('share').addEventListener('click', () => chosen && window.picker.choose(chosen));
addEventListener('keydown', (event) => event.key === 'Escape' && window.picker.choose(null));
