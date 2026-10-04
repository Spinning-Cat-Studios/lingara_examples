// Luba's destinations: a fixed table, never generated. Each phrase is what a
// passenger says to the pilot, in English and the example's own translations,
// keyed on the bare language subtag of the learner's target language.

export const PHRASE_LANGS = ["zh", "es", "fr"] as const;
export type PhraseLang = (typeof PHRASE_LANGS)[number];

export interface Dive {
  readonly name: string;
  readonly en: string;
  readonly phrases: Readonly<Record<PhraseLang, string>>;
}

export const DIVES: readonly Dive[] = [
  {
    name: "the Kelp Cathedral",
    en: "Take me to the kelp forest, please.",
    phrases: { zh: "请带我去海带森林。", es: "Lléveme al bosque de algas, por favor.", fr: "Emmenez-moi à la forêt de varech, s'il vous plaît." },
  },
  {
    name: "the Glass Wreck",
    en: "How deep is the shipwreck?",
    phrases: { zh: "沉船有多深？", es: "¿A qué profundidad está el naufragio?", fr: "À quelle profondeur est l'épave ?" },
  },
  {
    name: "the Lantern Trench",
    en: "Turn on the lights, please.",
    phrases: { zh: "请打开灯。", es: "Encienda las luces, por favor.", fr: "Allumez les lumières, s'il vous plaît." },
  },
  {
    name: "the Turtle Shelf",
    en: "Look, a sea turtle!",
    phrases: { zh: "看，一只海龟！", es: "¡Mira, una tortuga marina!", fr: "Regarde, une tortue de mer !" },
  },
  {
    name: "the Bubble Garden",
    en: "When do we go back up?",
    phrases: { zh: "我们什么时候回到水面？", es: "¿Cuándo volvemos a subir?", fr: "Quand est-ce qu'on remonte ?" },
  },
];

/** The next dive for a passenger who has logged `logged` dives: the table, in turn. */
export function nextDive(logged: number): Dive {
  return DIVES[logged % DIVES.length]!;
}

/** `zh-Hans` → `zh`; a language outside the table → `undefined`. */
export function phraseLang(targetLang: string): PhraseLang | undefined {
  const subtag = targetLang.split("-")[0]!.toLowerCase();
  return (PHRASE_LANGS as readonly string[]).includes(subtag) ? (subtag as PhraseLang) : undefined;
}
