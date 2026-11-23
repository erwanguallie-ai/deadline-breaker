"""Génère ui/css/theme-sombre.css : remplacements des classes de couleur Tailwind utilisées,
actifs lorsque <html> porte la classe « theme-sombre » (noir graphite neutre, sans teinte bleue)."""
import re, sys

html, js, sortie = sys.argv[1], sys.argv[2], sys.argv[3]
source = open(html, encoding='utf-8').read() + open(js, encoding='utf-8').read()

# Palette graphite : fond < cartes ; champs « en creux » ; éléments surélevés plus clairs
FOND, CARTE, CREUX, RELIEF, RELIEF2 = '#0b0b0c', '#161618', '#111113', '#1f1f22', '#2a2a2e'
# slate-300/400 servent de texte clair sur fond sombre (menu, en-têtes) : inchangés.
TEXTE = {'950': '#f4f4f5', '900': '#ededef', '800': '#dcdce0', '700': '#c4c4ca', '600': '#a1a1aa', '500': '#8b8b94'}
BORD = {'100': '#222225', '200': '#2a2a2e', '300': '#38383e'}
TEINTES = {  # (rgb du 400, hex du 300, hex du 400)
    'sky': ((56, 189, 248), '#7dd3fc', '#38bdf8'), 'rose': ((251, 113, 133), '#fda4af', '#fb7185'),
    'amber': ((251, 191, 36), '#fcd34d', '#fbbf24'), 'emerald': ((52, 211, 153), '#6ee7b7', '#34d399'),
    'indigo': ((129, 140, 248), '#a5b4fc', '#818cf8'), 'purple': ((192, 132, 252), '#d8b4fe', '#c084fc'),
    'blue': ((96, 165, 250), '#93c5fd', '#60a5fa'), 'red': ((248, 113, 113), '#fca5a5', '#f87171'),
    'orange': ((251, 146, 60), '#fdba74', '#fb923c'),
}

motif = re.compile(r'(?<![\w-])((?:hover:|focus:|group-hover:)?)(bg|text|border|divide|ring|from|to)-(white|slate-\d+|(?:%s)-\d+)(/\d+)?(?![\w-])' % '|'.join(TEINTES))
classes = sorted(set(m.group(0) for m in motif.finditer(source)))


def echapper(c):
    return re.sub(r'([:/.\[\]])', r'\\\1', c)


def rgba(rgb, a):
    return f'rgba({rgb[0]},{rgb[1]},{rgb[2]},{a})'


regles = []
for c in classes:
    m = motif.fullmatch(c)
    variante, prop, couleur, _op = m.group(1), m.group(2), m.group(3), m.group(4)
    sel = f'html.theme-sombre .{echapper(c)}' + (':hover' if variante == 'hover:' else ':focus' if variante == 'focus:' else '')
    if variante == 'group-hover:':
        sel = f'html.theme-sombre .group:hover .{echapper(c)}'
    decl = None
    if couleur == 'white':
        decl = {'bg': f'background-color:{CARTE}', 'text': None, 'border': f'border-color:{BORD["200"]}', 'ring': f'--tw-ring-color:{CARTE}'}.get(prop)
    elif couleur.startswith('slate-'):
        n = couleur.split('-')[1]
        if prop == 'bg':
            decl = {'50': f'background-color:{CREUX if variante == "" else RELIEF}', '100': f'background-color:{RELIEF}', '200': f'background-color:{RELIEF2}',
                    '800': 'background-color:#2c2c31', '900': 'background-color:#34343a', '950': None}.get(n)
            if variante == 'hover:' and n in ('50', '100'):
                decl = f'background-color:{RELIEF2}'
        elif prop == 'text':
            decl = f'color:{TEXTE[n]}' if n in TEXTE else None
            if variante == 'hover:' and n in ('600', '700', '800', '900'):
                decl = 'color:#f4f7fc'
        elif prop in ('border', 'divide'):
            couleur_bord = BORD.get(n, BORD['300'] if int(n) >= 300 and int(n) < 700 else None)
            if couleur_bord:
                decl = f'border-color:{couleur_bord}'
                if prop == 'divide':
                    sel = sel + ' > :not([hidden]) ~ :not([hidden])'
    else:
        famille, n = couleur.split('-')
        rgb, h300, h400 = TEINTES[famille]
        n = int(n)
        if prop == 'bg' and n <= 200:
            alpha = {50: .10, 100: .18, 200: .26}[n] + (.08 if variante == 'hover:' else 0)
            decl = f'background-color:{rgba(rgb, round(alpha, 2))}'
        elif prop == 'text' and n >= 600:
            decl = f'color:{h300 if n >= 700 else h400}'
        elif prop == 'border' and n <= 300:
            decl = f'border-color:{rgba(rgb, .38)}'
    if decl:
        regles.append(f'{sel}{{{decl}}}')

base = f"""/* Thème sombre « bleu nuit » - généré par outils/tailwind/gen_theme.py ; ne pas modifier à la main. */
html.theme-sombre, html.theme-sombre body, html.theme-sombre main {{ background-color:{FOND}; color:{TEXTE['800']}; color-scheme: dark; }}
html.theme-sombre body.bg-slate-50, html.theme-sombre main.bg-slate-50 {{ background-color:{FOND}; }}
html.theme-sombre #menu-lateral {{ background-color:#080809; border-color:#1c1c1f; }}
html.theme-sombre input, html.theme-sombre select, html.theme-sombre textarea {{ color:{TEXTE['900']}; color-scheme: dark; }}
html.theme-sombre input::placeholder, html.theme-sombre textarea::placeholder {{ color:#6b6b74; }}
html.theme-sombre option, html.theme-sombre optgroup {{ background-color:{CARTE}; color:{TEXTE['900']}; }}
html.theme-sombre .shadow-xs, html.theme-sombre .shadow-2xs {{ box-shadow: 0 1px 0 0 rgba(0,0,0,.35), 0 0 0 1px rgba(255,255,255,.02); }}
html.theme-sombre kbd {{ background-color:{RELIEF}; border-color:{BORD['300']}; color:{TEXTE['700']}; }}
html.theme-sombre ::-webkit-scrollbar-track {{ background:{FOND}; }}
html.theme-sombre ::-webkit-scrollbar-thumb {{ background:#34343a; }}
html.theme-sombre ::-webkit-scrollbar-thumb:hover {{ background:#4a4a52; }}
html.theme-sombre details summary {{ color:{TEXTE['500']}; }}
html.theme-sombre .poignee-ia {{ background-color:#232327; box-shadow:-4px 0 16px rgba(0,0,0,.5); }}
html.theme-sombre .poignee-ia:hover {{ background-color:#2e2e33; }}
html.theme-sombre .menu-groupe {{ color:#76767f; }}
"""
open(sortie, 'w', encoding='utf-8').write(base + '\n'.join(regles) + '\n')
print(f'{len(classes)} classes de couleur, {len(regles)} règles sombres')
