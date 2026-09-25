//! Prompts envoyés aux IA, dans la langue de l'interface. Les balises restent
//! identiques d'une langue à l'autre : seules les consignes sont traduites.

use crate::orchestrator::{AgentSpec, Exchange};

struct Texts {
    history_intro: &'static str,
    history_new: &'static str,
    debate_intro: &'static str,
    your_previous: &'static str,
    answer_of: &'static str,
    alone: &'static str,
    critique: &'static str,
    synthesis_intro: &'static str,
    synthesis_answers: &'static str,
    synthesis_instructions: &'static str,
}

const FR: Texts = Texts {
    history_intro: "Voici la discussion en cours entre l'utilisateur et un conseil de plusieurs IA \
        (la réponse du conseil est leur synthèse commune) :",
    history_new: "Nouveau message de l'utilisateur, auquel tu dois répondre en tenant compte de la discussion :",
    debate_intro: "Tu participes à une réflexion collective avec d'autres IA sur la demande suivante :",
    your_previous: "Ta réponse au tour précédent :",
    answer_of: "Réponse de",
    alone: "Les autres participants n'ont pas pu répondre. Relis ta réponse d'un œil critique, \
        corrige ses erreurs et ses oublis, puis donne ta réponse révisée.",
    critique: "Analyse les réponses des autres participants : ce qui est juste, les erreurs, les \
        oublis et les points de désaccord avec ta propre réponse. Sois honnête : si un autre a \
        raison, adopte son idée ; si tu penses avoir raison, défends-la avec des arguments. Puis \
        donne ta réponse révisée et complète à la demande d'origine.\n\nFormat attendu :\n\n\
        ## Retours\n(ta critique des autres réponses)\n\n## Réponse révisée\n(ta réponse améliorée)",
    synthesis_intro: "Plusieurs IA ont travaillé sur la demande suivante :",
    synthesis_answers: "Voici leurs réponses finales :",
    synthesis_instructions: "Produis une réponse finale unique qui combine le meilleur de ces \
        réponses, corrige leurs erreurs et tranche leurs désaccords en expliquant brièvement \
        pourquoi. Réponds directement à la demande ; ne commente les participants que pour \
        signaler un désaccord important. Réponds en français.",
};

const EN: Texts = Texts {
    history_intro: "Here is the ongoing discussion between the user and a council of several AIs \
        (the council's answer is their shared synthesis):",
    history_new: "New message from the user, which you must answer in light of the discussion:",
    debate_intro: "You are taking part in a collective reflection with other AIs on the following request:",
    your_previous: "Your answer in the previous round:",
    answer_of: "Answer from",
    alone: "The other participants could not answer. Review your answer critically, fix its \
        mistakes and gaps, then give your revised answer.",
    critique: "Analyze the other participants' answers: what is right, the mistakes, the gaps and \
        the points where they disagree with your own answer. Be honest: if someone else is right, \
        adopt their idea; if you think you are right, defend it with arguments. Then give your \
        revised, complete answer to the original request.\n\nExpected format:\n\n\
        ## Feedback\n(your critique of the other answers)\n\n## Revised answer\n(your improved answer)",
    synthesis_intro: "Several AIs worked on the following request:",
    synthesis_answers: "Here are their final answers:",
    synthesis_instructions: "Produce a single final answer that combines the best of these \
        answers, fixes their mistakes and settles their disagreements, briefly explaining why. \
        Answer the request directly; only mention the participants to flag a significant \
        disagreement. Answer in English.",
};

const ES: Texts = Texts {
    history_intro: "Esta es la conversación en curso entre el usuario y un consejo de varias IA \
        (la respuesta del consejo es su síntesis común):",
    history_new: "Nuevo mensaje del usuario, al que debes responder teniendo en cuenta la conversación:",
    debate_intro: "Participas en una reflexión colectiva con otras IA sobre la siguiente petición:",
    your_previous: "Tu respuesta en la ronda anterior:",
    answer_of: "Respuesta de",
    alone: "Los demás participantes no pudieron responder. Revisa tu respuesta con ojo crítico, \
        corrige sus errores y omisiones, y da tu respuesta revisada.",
    critique: "Analiza las respuestas de los demás participantes: lo que es correcto, los errores, \
        las omisiones y los puntos en los que discrepan de tu propia respuesta. Sé honesto: si otro \
        tiene razón, adopta su idea; si crees tener razón, defiéndela con argumentos. Después da tu \
        respuesta revisada y completa a la petición original.\n\nFormato esperado:\n\n\
        ## Comentarios\n(tu crítica de las otras respuestas)\n\n## Respuesta revisada\n(tu respuesta mejorada)",
    synthesis_intro: "Varias IA trabajaron en la siguiente petición:",
    synthesis_answers: "Estas son sus respuestas finales:",
    synthesis_instructions: "Elabora una única respuesta final que combine lo mejor de estas \
        respuestas, corrija sus errores y resuelva sus desacuerdos explicando brevemente por qué. \
        Responde directamente a la petición; menciona a los participantes solo para señalar un \
        desacuerdo importante. Responde en español.",
};

const AR: Texts = Texts {
    history_intro: "إليك النقاش الجاري بين المستخدم ومجلس من عدة أنظمة ذكاء اصطناعي \
        (إجابة المجلس هي خلاصتهم المشتركة):",
    history_new: "رسالة جديدة من المستخدم، عليك الرد عليها مع مراعاة النقاش:",
    debate_intro: "أنت تشارك في تفكير جماعي مع أنظمة ذكاء اصطناعي أخرى حول الطلب التالي:",
    your_previous: "إجابتك في الجولة السابقة:",
    answer_of: "إجابة",
    alone: "لم يتمكن المشاركون الآخرون من الإجابة. راجع إجابتك بعين ناقدة، وصحّح أخطاءها \
        ونواقصها، ثم قدّم إجابتك المنقحة.",
    critique: "حلّل إجابات المشاركين الآخرين: ما هو صحيح، والأخطاء، والنواقص، ونقاط الخلاف مع \
        إجابتك. كن صادقًا: إذا كان غيرك محقًا فتبنَّ فكرته، وإذا رأيت أنك محق فدافع عنها بالحجج. \
        ثم قدّم إجابتك المنقحة والكاملة على الطلب الأصلي.\n\nالتنسيق المطلوب:\n\n\
        ## الملاحظات\n(نقدك للإجابات الأخرى)\n\n## الإجابة المنقحة\n(إجابتك المحسّنة)",
    synthesis_intro: "عملت عدة أنظمة ذكاء اصطناعي على الطلب التالي:",
    synthesis_answers: "إليك إجاباتها النهائية:",
    synthesis_instructions: "قدّم إجابة نهائية واحدة تجمع أفضل ما في هذه الإجابات، وتصحّح \
        أخطاءها، وتحسم خلافاتها مع شرح موجز للسبب. أجب عن الطلب مباشرة، ولا تذكر المشاركين إلا \
        للإشارة إلى خلاف مهم. أجب باللغة العربية.",
};

fn texts(lang: &str) -> &'static Texts {
    match lang {
        "en" => &EN,
        "es" => &ES,
        "ar" => &AR,
        _ => &FR,
    }
}

/// Replace le nouveau message dans le fil de la discussion.
pub fn with_history(lang: &str, history: &[Exchange], prompt: &str) -> String {
    if history.is_empty() {
        return prompt.to_string();
    }
    let t = texts(lang);
    let mut s = format!("{}\n\n<history>\n", t.history_intro);
    for (i, ex) in history.iter().enumerate() {
        s.push_str(&format!(
            "<exchange n=\"{n}\">\n<user>\n{prompt}\n</user>\n\
             <council_answer>\n{answer}\n</council_answer>\n</exchange>\n",
            n = i + 1,
            prompt = ex.prompt,
            answer = ex.answer
        ));
    }
    s.push_str(&format!("</history>\n\n{}\n\n{prompt}", t.history_new));
    s
}

/// Prompt d'un tour de revue croisée : la demande, la réponse précédente de
/// l'agent et celles des autres participants.
pub fn debate(
    lang: &str,
    question: &str,
    me: &AgentSpec,
    previous: &[(AgentSpec, String)],
) -> String {
    let t = texts(lang);
    let mut s = format!(
        "{}\n\n<request>\n{question}\n</request>\n\n",
        t.debate_intro
    );

    if let Some((_, own)) = previous.iter().find(|(a, _)| a.key == me.key) {
        s.push_str(&format!(
            "{}\n\n<your_answer>\n{own}\n</your_answer>\n\n",
            t.your_previous
        ));
    }

    let others: Vec<_> = previous.iter().filter(|(a, _)| a.key != me.key).collect();
    for (agent, answer) in &others {
        s.push_str(&format!(
            "{} {label} :\n\n<answer author=\"{label}\">\n{answer}\n</answer>\n\n",
            t.answer_of,
            label = agent.label
        ));
    }

    s.push_str(if others.is_empty() {
        t.alone
    } else {
        t.critique
    });
    s
}

/// Prompt de synthèse : fusionner les réponses finales en une seule.
pub fn synthesis(lang: &str, question: &str, answers: &[(AgentSpec, String)]) -> String {
    let t = texts(lang);
    let mut s = format!(
        "{}\n\n<request>\n{question}\n</request>\n\n{}\n\n",
        t.synthesis_intro, t.synthesis_answers
    );
    for (agent, answer) in answers {
        s.push_str(&format!(
            "<answer author=\"{label}\">\n{answer}\n</answer>\n\n",
            label = agent.label
        ));
    }
    s.push_str(t.synthesis_instructions);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(key: &str, label: &str) -> AgentSpec {
        AgentSpec {
            key: key.into(),
            label: label.into(),
            provider: "claude-cli".into(),
            model: String::new(),
            effort: String::new(),
        }
    }

    #[test]
    fn debate_separates_own_answer_from_others() {
        let a = spec("a", "Claude");
        let b = spec("b", "Gemini");
        let previous = vec![(a.clone(), "réponse A".into()), (b, "réponse B".into())];
        let prompt = debate("fr", "Q ?", &a, &previous);
        assert!(prompt.contains("<your_answer>\nréponse A\n</your_answer>"));
        assert!(prompt.contains("<answer author=\"Gemini\">\nréponse B\n</answer>"));
        assert!(!prompt.contains("author=\"Claude\""));
        assert!(prompt.contains("## Réponse révisée"));
    }

    #[test]
    fn debate_alone_asks_for_self_review() {
        let a = spec("a", "Claude");
        let prompt = debate("fr", "Q ?", &a, &[(a.clone(), "réponse A".into())]);
        assert!(prompt.contains("n'ont pas pu répondre"));
    }

    #[test]
    fn prompts_follow_the_interface_language() {
        let a = spec("a", "Claude");
        let b = spec("b", "Gemini");
        let previous = vec![(a.clone(), "A".into()), (b, "B".into())];
        assert!(debate("en", "Q?", &a, &previous).contains("## Revised answer"));
        assert!(debate("es", "Q?", &a, &previous).contains("## Respuesta revisada"));
        assert!(debate("ar", "Q?", &a, &previous).contains("## الإجابة المنقحة"));
        assert!(synthesis("en", "Q?", &previous).ends_with("Answer in English."));
        // Langue inconnue : on retombe sur le français.
        assert!(debate("de", "Q?", &a, &previous).contains("## Réponse révisée"));
    }

    #[test]
    fn history_is_prepended_in_order() {
        assert_eq!(with_history("fr", &[], "Salut"), "Salut");
        let history = vec![
            Exchange {
                prompt: "Q1".into(),
                answer: "R1".into(),
            },
            Exchange {
                prompt: "Q2".into(),
                answer: "R2".into(),
            },
        ];
        let prompt = with_history("fr", &history, "Q3");
        let (q1, r2, q3) = (
            prompt.find("Q1").unwrap(),
            prompt.find("R2").unwrap(),
            prompt.find("Q3").unwrap(),
        );
        assert!(q1 < r2 && r2 < q3);
        assert!(prompt.contains("<exchange n=\"2\">"));
    }

    #[test]
    fn synthesis_lists_every_answer() {
        let answers = vec![
            (spec("a", "Claude"), "A".into()),
            (spec("b", "Gemini"), "B".into()),
        ];
        let prompt = synthesis("fr", "Q ?", &answers);
        assert!(prompt.contains("author=\"Claude\""));
        assert!(prompt.contains("author=\"Gemini\""));
    }
}
