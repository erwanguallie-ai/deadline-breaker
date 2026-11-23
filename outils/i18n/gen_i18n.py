import json, sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from dico_ui import EXACT, GABARITS
sortie = sys.argv[1]
exact = {k.strip() if k.strip() else k: v for k, v in EXACT.items()}
contenu = ("/* Dictionnaire anglais de l'interface Deadline Breaker (généré par outils/i18n/gen_i18n.py).\n"
           "   Clé : texte français affiché ; {} : valeur variable. Le droit reste français. */\n"
           "window.I18N_EN = " + json.dumps({'exact': exact, 'gabarits': GABARITS}, ensure_ascii=False, indent=0) + ";\n")
open(sortie, 'w', encoding='utf-8').write(contenu)
print(len(exact), 'textes,', len(GABARITS), 'gabarits')
