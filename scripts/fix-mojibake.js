// Fix mojibake from UTF-8 misread as Windows-1252 then re-saved as UTF-8.
// Uses a fixed mapping of common mojibake sequences -> correct UTF-8 chars.
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..', 'docs', 'site', 'pages');

// Map: mojibake sequence (already-decoded as the wrong characters in the file)
// -> correct character. Order matters: longer sequences before shorter ones
// that share a prefix.
const REPLACEMENTS = [
    // Box drawing
    ['\u00E2\u2022\u00B0\u00E2\u201D\u20AC', '\u2570\u2500'], // ╰─
    ['\u00E2\u2022\u00AD\u00E2\u201D\u20AC', '\u256D\u2500'], // ╭─
    ['\u00E2\u2022\u00B7', '\u2577'], // ╷
    ['\u00E2\u2022\u00B5', '\u2575'], // ╵
    ['\u00E2\u201D\u201A', '\u2502'], // │
    ['\u00E2\u201D\u20AC', '\u2500'], // ─
    ['\u00E2\u2022\u00B0', '\u2570'], // ╰
    ['\u00E2\u2022\u00AD', '\u256D'], // ╭
    ['\u00E2\u2022\u00AE', '\u256E'], // ╮
    ['\u00E2\u2022\u00AF', '\u256F'], // ╯
    // Geometric shapes
    ['\u00E2\u2014\u2039', '\u25CB'], // ○
    ['\u00E2\u2013\u00A0', '\u25A0'], // ■
    // Punctuation
    ['\u00E2\u20AC\u201D', '\u2014'], // —
    ['\u00E2\u20AC\u201C', '\u2013'], // –
    ['\u00E2\u20AC\u00A2', '\u2022'], // •
    ['\u00E2\u20AC\u02DC', '\u2018'], // '
    ['\u00E2\u20AC\u2122', '\u2019'], // '
    ['\u00E2\u20AC\u0153', '\u201C'], // "
    ['\u00E2\u20AC\u009D', '\u201D'], // " (rare)
    ['\u00E2\u20AC\u00A6', '\u2026'], // …
    ['\u00E2\u2020\u2019', '\u2192'], // →
    // Stray non-breaking-space mojibake
    ['\u00C2\u00A0', '\u00A0'], // NBSP
];

function fixText(s) {
    let out = s;
    for (const [from, to] of REPLACEMENTS) {
        if (out.includes(from)) out = out.split(from).join(to);
    }
    return out;
}

function walk(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const p = path.join(dir, entry.name);
        if (entry.isDirectory()) walk(p);
        else if (/\.mdx?$/.test(entry.name)) processFile(p);
    }
}

function processFile(file) {
    const orig = fs.readFileSync(file, 'utf8');
    const fixed = fixText(orig);
    if (fixed !== orig) {
        fs.writeFileSync(file, fixed, 'utf8');
        console.log('fixed:', path.relative(process.cwd(), file));
    }
}

walk(ROOT);
console.log('done');
