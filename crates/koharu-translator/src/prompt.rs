use std::fmt::Write;

use anyhow::Context;
use indoc::indoc;
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::{Value, json};

use crate::{
    Language, Translated, TranslationCharacter, TranslationCharacterUpdate, TranslationContext,
    TranslationProposals, TranslationRequest, TranslationTerm, TranslationTermUpdate,
};

pub(crate) fn prompts(request: &TranslationRequest) -> anyhow::Result<(String, String)> {
    if request.describe {
        let mut input = json!({ "characters": request.characters });
        if !request.previous_descriptions.is_empty() {
            input["previous_pages"] = json!(request.previous_descriptions);
        }
        let user =
            serde_json::to_string(&input).context("failed to serialize description input")?;
        return Ok((description_system_prompt(request), user));
    }
    let input = TranslationInput {
        scene_description: request.scene_description.as_deref(),
        source_language: request.source_language,
        target_language: request.target_language,
        context: &request.context,
        characters: &request.characters,
        terms: &request.terms,
        known_sources: &request.known_sources,
        note: request.note.as_deref(),
        previous_translation: request.previous_translation.as_deref(),
        segments: request
            .segments
            .iter()
            .enumerate()
            .map(|(id, text)| TranslationInputSegment { id, text })
            .collect(),
    };
    let user = serde_json::to_string(&input).context("failed to serialize translation input")?;
    Ok((translation_system_prompt(request), user))
}

pub(crate) fn translations(
    provider: &str,
    text: &str,
    source_segments: &[String],
) -> anyhow::Result<Translated> {
    let text = unfence(text);
    // A request without segments is a description request: there is nothing to translate.
    if source_segments.is_empty() {
        return description(provider, text);
    }
    let output = parse_output(text).with_context(|| {
        format!(
            "{provider} returned invalid translation JSON for {} segments; response was: {}",
            source_segments.len(),
            snippet(text),
        )
    })?;
    let mut translations = source_segments.to_vec();
    let mut translated = vec![false; source_segments.len()];

    for translation in output.translations {
        if translation.id < translations.len() && !translated[translation.id] {
            translations[translation.id] = translation.text;
            translated[translation.id] = true;
        }
    }

    Ok(Translated {
        segments: translations,
        proposals: proposals(&output.proposals),
        // Models send `null` (or nothing) for it; that must never fail the translation.
        explanation: output
            .explanation
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned),
        description: None,
    })
}

fn description(provider: &str, text: &str) -> anyhow::Result<Translated> {
    let value: Value = serde_json::from_str(text).with_context(|| {
        format!(
            "{provider} returned invalid description JSON; response was: {}",
            snippet(text)
        )
    })?;
    // Models send `null` for a field the schema says is a string; that is "no description".
    let description = value
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    // Characters the model could not match to the sheet; read like translation proposals.
    let proposals = proposals(&json!({ "characters": value.get("characters") }));
    Ok(Translated {
        description,
        proposals,
        ..Translated::default()
    })
}

/// Reads proposals entry by entry: proposals are a bonus, so anything malformed is dropped
/// and never fails the translation.
fn proposals(value: &Value) -> TranslationProposals {
    fn entries<T: de::DeserializeOwned>(value: &Value, key: &str) -> Vec<T> {
        value
            .get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        // Models send `null` for unknown fields even where the schema says
                        // string; treat it as absent so `#[serde(default)]` applies.
                        let mut item = item.clone();
                        if let Value::Object(fields) = &mut item {
                            fields.retain(|_, value| !value.is_null());
                        }
                        serde_json::from_value(item).ok()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    let mut proposals = TranslationProposals {
        characters: entries(value, "characters"),
        terms: entries(value, "terms"),
        character_updates: entries(value, "character_updates"),
        term_updates: entries(value, "term_updates"),
    };
    proposals
        .character_updates
        .retain(|update: &TranslationCharacterUpdate| !update.source_name.trim().is_empty());
    proposals
        .term_updates
        .retain(|update: &TranslationTermUpdate| !update.source.trim().is_empty());
    proposals
        .characters
        .retain(|character: &TranslationCharacter| !character.source_name.trim().is_empty());
    proposals
        .terms
        .retain(|term: &TranslationTerm| !term.source.trim().is_empty());
    proposals
}

/// Keeps a model response readable in a log line without truncating so hard
/// that the shape of the failure is lost.
///
/// Responses are usually pretty-printed, so line breaks and other control
/// characters are escaped rather than passed through: the snippet is formatted
/// into an error context, and one failure should stay one line.
pub(crate) fn snippet(text: &str) -> String {
    const LIMIT: usize = 2000;
    let trimmed = text.trim();
    let (visible, elided) = match trimmed.char_indices().nth(LIMIT) {
        Some((end, _)) => (&trimmed[..end], true),
        None => (trimmed, false),
    };

    let mut output = String::with_capacity(visible.len());
    for character in visible.chars() {
        match character {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            control if control.is_control() => {
                let _ = write!(output, "\\u{{{:04x}}}", control as u32);
            }
            character => output.push(character),
        }
    }
    if elided {
        let _ = write!(output, "… ({} bytes total)", trimmed.len());
    }
    output
}

/// The schema for `request`: the explanation field exists only for one-box requests, so a
/// full-page request is unchanged.
pub(crate) fn request_schema(request: &TranslationRequest) -> Value {
    if request.describe {
        return json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "Who is present, what is happening, and the mood of the page."
                },
                "characters": {
                    "type": "array",
                    "description": "Characters on the page that match nobody on the character sheet.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "source_name": { "type": "string" },
                            "appearance": { "type": "string" }
                        },
                        "required": ["source_name", "appearance"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["description", "characters"],
            "additionalProperties": false
        });
    }
    let mut schema = output_schema(request.segments.len());
    if request.explain {
        schema["properties"]["explanation"] = json!({
            "type": "string",
            "description": "A short explanation of why the line was translated this way."
        });
        schema["required"] = json!(["translations", "proposals", "explanation"]);
    }
    schema
}

pub(crate) fn output_schema(expected: usize) -> Value {
    json!({
        "type": "object",
        "properties": {
            "translations": {
                "type": "array",
                "minItems": expected,
                "maxItems": expected,
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "integer",
                            "minimum": 0,
                            "maximum": expected.saturating_sub(1),
                            "description": "The ID copied from the corresponding input segment."
                        },
                        "text": {
                            "type": "string",
                            "description": "The translation of the input segment with this ID."
                        }
                    },
                    "required": ["id", "text"],
                    "additionalProperties": false
                }
            },
            // Required in the schema because strict structured-output providers reject optional
            // properties; the parser still accepts a missing or malformed `proposals`.
            "proposals": {
                "type": "object",
                "description": "New recurring characters and terms worth adding to the project. Use empty arrays when there are none.",
                "properties": {
                    "characters": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "source_name": { "type": "string" },
                                "target_name": { "type": "string" },
                                "speech_style": { "type": "string" },
                                "appearance": { "type": "string" },
                                "notes": { "type": "string" }
                            },
                            "required": ["source_name", "target_name", "speech_style", "appearance", "notes"],
                            "additionalProperties": false
                        }
                    },
                    "terms": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "source": { "type": "string" },
                                "target": { "type": "string" },
                                "note": { "type": "string" }
                            },
                            "required": ["source", "target", "note"],
                            "additionalProperties": false
                        }
                    },
                    // Strict structured outputs need every property required, so "unchanged"
                    // is an explicit null rather than an absent field.
                    "character_updates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "source_name": { "type": "string" },
                                "target_name": { "type": ["string", "null"] },
                                "speech_style": { "type": ["string", "null"] },
                                "appearance": { "type": ["string", "null"] },
                                "notes": { "type": ["string", "null"] }
                            },
                            "required": ["source_name", "target_name", "speech_style", "appearance", "notes"],
                            "additionalProperties": false
                        }
                    },
                    "term_updates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "source": { "type": "string" },
                                "target": { "type": ["string", "null"] },
                                "note": { "type": ["string", "null"] }
                            },
                            "required": ["source", "target", "note"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["characters", "terms", "character_updates", "term_updates"],
                "additionalProperties": false
            }
        },
        "required": ["translations", "proposals"],
        "additionalProperties": false
    })
}

fn description_system_prompt(request: &TranslationRequest) -> String {
    let mut prompt = format!(
        indoc! {"
            You describe one manga page so a translator knows the scene.

            Requirements:
            - Look at the attached page image.
            - Say who is present, what is happening, and the mood. Note who seems to be speaking to whom.
            - Write the description only in {target}, in a few plain sentences. Describe what is shown; do not translate or transcribe the dialogue. Ignore all text, speech bubbles and sound effects; describe only who is there, where, what they are doing and their expressions.
            - List in `characters` each person who matches nobody on the character sheet: `source_name` as the page names them, and their `appearance`. Skip people the page does not name. Use an empty array when everyone is matched or unnamed.
            - Return only a JSON object with a `description` string and a `characters` array.
        "},
        target = request.target_language,
    )
    .trim_end()
    .to_owned();
    if !request.characters.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(indoc! {"
            Character sheet requirements:
            The `characters` array lists the project's recurring characters with their `appearance` and `notes`.
            Compare everyone on the page with those entries. When someone matches a character, call them by that character's `target_name` (for example 'Adachi') and do not describe their looks; the translator already knows them. Describe by looks only people who match nobody.
        "}.trim_end());
    }
    if !request.previous_descriptions.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(indoc! {"
            Previous pages requirements:
            The `previous_pages` array holds the descriptions of the pages just before this one, oldest first, each with its page number.
            Use them only for continuity: who is where and what changed. Describe only the attached page; do not repeat or summarize them.
        "}.trim_end());
    }
    if let Some(instructions) = request
        .instructions
        .as_deref()
        .map(str::trim)
        .filter(|instructions| !instructions.is_empty())
    {
        prompt.push_str("\n\nAdditional instructions:\n");
        prompt.push_str(instructions);
    }
    prompt
}

fn translation_system_prompt(request: &TranslationRequest) -> String {
    let source = request
        .source_language
        .map(|language| language.to_string())
        .unwrap_or_else(|| "the detected source language".to_owned());
    let mut prompt = format!(
        indoc! {"
            You are a professional manga translator.

            Translation requirements:
            - Translate every input segment from {source} into natural {target}.
            - Preserve meaning, character voice, emotional tone, relationship nuance, emphasis, and sound effects.
            - Localize idioms and sound effects naturally while keeping wording concise enough for speech bubbles.
            - Use surrounding segments only for disambiguation and continuity; never merge or split segments.
            - Write every translated `text` value only in {target}; do not include source text, notes, explanations, or alternatives.
            - Never preserve or repeat original-language text; translate names, terms, and sound effects using natural {target} conventions.

            Output requirements:
            - Each input segment has a numeric `id`.
            - Return only a JSON object whose `translations` array contains one object with `id` and translated `text` for every input segment.
            - Copy every input ID exactly once; order does not matter.
            - Never merge, split, omit, duplicate, or add segments.
        "},
        source = source,
        target = request.target_language,
    )
    .trim_end()
    .to_owned();

    if !request.context.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(indoc! {"
            Context requirements:
            Use the supplied context only to preserve terminology, character voice, and dialogue continuity.
            Do not translate or return the context entries.
            Context entries with user_corrected set to true were corrected by the user; treat them as authoritative.
        "}.trim_end());
    }

    if !request.characters.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(indoc! {"
            Character sheet requirements:
            The `characters` array lists the project's recurring characters; it is sent with every page.
            Each entry has `source_name`, `target_name`, `speech_style`, `appearance`, and `notes`.
            Always render a character's name as `target_name` when it is set.
            Follow `speech_style` for that character's dialogue, and use `appearance` and `notes` to resolve who is speaking and how characters relate.
            Do not translate or return the character entries.
        "}.trim_end());
    }

    if !request.terms.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(indoc! {"
            Term list requirements:
            The `terms` array lists project terms that occur on this page.
            Each entry has `source`, `target`, and `note`.
            Whenever `source` appears in a segment, render it exactly as `target`, without paraphrasing or inflecting it.
            Use `note`, when set, only to understand the context; never output it.
            Do not translate or return the term entries.
        "}.trim_end());
    }

    if request
        .characters
        .iter()
        .any(|character| character.tentative)
        || request.terms.iter().any(|term| term.tentative)
    {
        prompt.push_str("\n\n");
        prompt.push_str(
            "Entries with tentative set to true are unreviewed proposals; use them as hints, but they may be wrong.",
        );
    }

    prompt.push_str("\n\n");
    prompt.push_str(indoc! {"
        Proposal requirements:
        The `proposals` object may list new `characters` (source_name, target_name, speech_style, appearance, notes) and new `terms` (source, target, note).
        Propose only names and terms that recur or will clearly recur, and only when their source is not already in the character sheet or term list.
        Never propose a source listed in `known_sources`; those are pending or rejected.
        To change an existing character or term, use `character_updates` (source_name plus target_name, speech_style, appearance, notes) or `term_updates` (source plus target, note). source_name or source must match the existing entry exactly. Set a field to null to leave it unchanged.
        Propose an update only for a clearly new fact about an existing entry, such as a newly noticed speech quirk or a name correction; never restate current values.
        Use empty arrays when there is nothing to propose. Proposals never replace the translations.
    "}.trim_end());

    if request.scene_description.is_some() {
        prompt.push_str("\n\n");
        prompt.push_str(
            indoc! {"
            Scene description requirements:
            `scene_description` says who is present, what is happening, and the mood on this page.
            Use it to resolve who speaks to whom and the tone; do not translate or return it.
        "}
            .trim_end(),
        );
    }

    if request.explain {
        prompt.push_str("\n\n");
        prompt.push_str(&format!(
            indoc! {"
                Explanation requirements:
                Also return an `explanation` string: a short note on why you translated the line this way (reading of the source, wordplay, alternatives considered), written in {target}.
                The explanation never goes into `text`.
            "},
            target = request.target_language,
        ));
    }

    if request.note.is_some() {
        prompt.push_str("\n\n");
        prompt.push_str(indoc! {"
            Note requirements:
            The `note` is guidance from the user for this request only; follow it.
            When `previous_translation` is present it is the earlier attempt; improve on it using the note instead of starting over.
        "}.trim_end());
    }

    if request.image.is_some() {
        prompt.push_str("\n\n");
        prompt.push_str(indoc! {"
            Image requirements:
            Use the attached original page image as visual context for speaker identity, tone, layout, and ambiguous OCR.
            Translate only the supplied segments; do not add text seen in the image that is absent from the input segments.
        "}.trim_end());
    }

    if let Some(instructions) = request
        .instructions
        .as_deref()
        .map(str::trim)
        .filter(|instructions| !instructions.is_empty())
    {
        prompt.push_str("\n\nAdditional instructions:\n");
        prompt.push_str(instructions);
    }
    prompt
}

#[derive(Serialize)]
struct TranslationInput<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    scene_description: Option<&'a str>,
    source_language: Option<Language>,
    target_language: Language,
    context: &'a [TranslationContext],
    characters: &'a [TranslationCharacter],
    terms: &'a [TranslationTerm],
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    known_sources: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_translation: Option<&'a str>,
    segments: Vec<TranslationInputSegment<'a>>,
}

#[derive(Serialize)]
struct TranslationInputSegment<'a> {
    id: usize,
    text: &'a str,
}

/// Models wrap the object in a Markdown code fence (` ```json ... ``` `). Strip exactly one fence
/// that spans the whole reply; prose around the JSON stays rejected.
fn unfence(text: &str) -> &str {
    text.trim()
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
        .map_or(text, |inner| {
            inner
                .trim_start_matches(|c: char| c.is_ascii_alphanumeric())
                .trim()
        })
}

/// Strict parse first. Models sometimes emit a stray bracket after a valid `translations`
/// array, so on failure salvage each top-level field on its own: the translations must
/// survive, and proposals are a bonus that may be lost.
fn parse_output(text: &str) -> serde_json::Result<TranslationOutput> {
    let error = match serde_json::from_str::<TranslationOutput>(text) {
        Ok(output) => return Ok(output),
        Err(error) => error,
    };
    // Wrapped output (code fences, prose) stays rejected; only a bare object is salvaged.
    let trimmed = text.trim();
    if !(trimmed.starts_with('{') && trimmed.ends_with('}')) {
        return Err(error);
    }
    // ponytail: finds the first `"key":` textually, so a segment containing `"translations":`
    // could confuse it; upgrade to a lenient JSON parser if that ever shows up.
    fn first_value<T: de::DeserializeOwned>(text: &str, key: &str) -> Option<T> {
        let after = &text[text.find(&format!("\"{key}\""))? + key.len() + 2..];
        let rest = after.trim_start().strip_prefix(':')?;
        serde_json::Deserializer::from_str(rest)
            .into_iter::<T>()
            .next()?
            .ok()
    }
    match first_value(text, "translations") {
        Some(translations) => Ok(TranslationOutput {
            translations,
            proposals: first_value(text, "proposals").unwrap_or(Value::Null),
            explanation: first_value(text, "explanation").unwrap_or(Value::Null),
        }),
        None => Err(error),
    }
}

#[derive(Debug, Deserialize)]
struct TranslationOutput {
    translations: Vec<TranslationOutputSegment>,
    #[serde(default)]
    proposals: Value,
    #[serde(default)]
    explanation: Value,
}

#[derive(Debug, Deserialize)]
struct TranslationOutputSegment {
    #[serde(deserialize_with = "deserialize_segment_id")]
    id: usize,
    text: String,
}

fn deserialize_segment_id<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SegmentId {
        Number(usize),
        String(String),
    }

    match SegmentId::deserialize(deserializer)? {
        SegmentId::Number(id) => Ok(id),
        SegmentId::String(id) => id.trim().parse().map_err(de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PreviousDescription;

    #[test]
    fn snippet_keeps_a_response_on_one_line() {
        let flattened = snippet("{\n  \"translations\": [\n\t\"hello\"\n  ]\n}");

        assert!(!flattened.contains('\n'), "{flattened}");
        assert!(!flattened.contains('\t'), "{flattened}");
        assert_eq!(flattened, r#"{\n  "translations": [\n\t"hello"\n  ]\n}"#);
    }

    #[test]
    fn snippet_escapes_other_control_characters() {
        assert_eq!(snippet("before\u{7}after"), r"before\u{0007}after");
    }

    #[test]
    fn snippet_reports_the_length_it_elided() {
        let flattened = snippet(&"x".repeat(2_500));

        assert!(flattened.starts_with(&"x".repeat(2_000)));
        assert!(
            flattened.ends_with("\u{2026} (2500 bytes total)"),
            "{flattened}"
        );
    }

    #[test]
    fn parses_plain_json() {
        let source = ["one".to_owned(), "two".to_owned()];
        let response = r#"{"translations":[{"id":0,"text":"hello"},{"id":1,"text":"world"}]}"#;

        assert_eq!(
            translations("test", response, &source).unwrap().segments,
            ["hello", "world"]
        );
    }

    #[test]
    fn accepts_one_code_fence_around_the_object() {
        let source = ["one".to_owned()];
        let object = r#"{"translations":[{"id":0,"text":"hello"}]}"#;
        for response in [
            format!("```json\n{object}\n```"),
            format!("```\n{object}\n```"),
            format!("  ```JSON\n{object}```\n"),
        ] {
            assert_eq!(
                translations("test", &response, &source).unwrap().segments,
                ["hello"],
                "{response}"
            );
        }
        let described = translations(
            "test",
            "```json\n{\"description\":\"Two friends.\",\"characters\":[]}\n```",
            &[],
        )
        .unwrap();
        assert_eq!(described.description.as_deref(), Some("Two friends."));
    }

    #[test]
    fn rejects_wrapped_and_malformed_json() {
        let source = ["one".to_owned(), "two".to_owned()];
        for response in [
            "Here it is:\n```json\n{\"translations\":[{\"id\":0,\"text\":\"hello\"},{\"id\":1,\"text\":\"world\"}]}\n```",
            r#"{translations: [{id: 0, text: 'hello'}, {id: 1, text: 'world'},],}"#,
            r#"Here is the result: {"translations": [{"id": 0, "text": "hello"}, {"id": 1, "text": "world"},]}"#,
            "{\"translations\":[{\"id\":0,\"text\":\"hello\"},{\"id\":1,\"text\":\"world\"",
        ] {
            assert!(
                translations("test", response, &source).is_err(),
                "{response}"
            );
        }
    }

    #[test]
    fn parse_error_preserves_the_root_failure_and_response() {
        let source = ["one".to_owned()];
        let response = "{\n  \"translations\": [{\"id\": 0}]\n}";
        let error = format!(
            "{:#}",
            translations("test", response, &source).expect_err("missing text should fail")
        );

        assert!(error.contains("missing field `text`"), "{error}");
        assert!(
            error.contains(r#"response was: {\n  "translations": [{"id": 0}]\n}"#),
            "{error}"
        );
    }

    #[test]
    fn restores_input_order_from_ids() {
        let source = ["one".to_owned(), "two".to_owned()];
        let response = r#"{"translations":[{"id":1,"text":"world"},{"id":0,"text":"hello"}]}"#;
        assert_eq!(
            translations("test", response, &source).unwrap().segments,
            ["hello", "world"]
        );
    }

    #[test]
    fn tolerates_duplicate_missing_and_out_of_range_ids() {
        let source = ["one".to_owned(), "two".to_owned()];
        let short = r#"{"translations":[{"id":1,"text":"world"}]}"#;
        assert_eq!(
            translations("test", short, &source).unwrap().segments,
            ["one", "world"]
        );

        let response = concat!(
            r#"{"translations":["#,
            r#"{"id":0,"text":"hello"},"#,
            r#"{"id":0,"text":"duplicate"},"#,
            r#"{"id":9,"text":"extra"}"#,
            "]}"
        );
        assert_eq!(
            translations("test", response, &source).unwrap().segments,
            ["hello", "two"]
        );
    }

    #[test]
    fn prompt_payload_contains_ordered_context() {
        let request = TranslationRequest::new(["new"], Language::English)
            .with_context([TranslationContext::new("old", "previous")]);
        let (_, user) = prompts(&request).unwrap();
        let input: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(input["context"][0]["source"], "old");
        assert_eq!(input["context"][0]["translation"], "previous");
        assert_eq!(input["segments"][0]["id"], 0);
        assert_eq!(input["segments"][0]["text"], "new");
    }

    #[test]
    fn description_request_sends_the_character_sheet_and_asks_for_a_description() {
        let request = TranslationRequest::describing(Language::English).with_characters([
            TranslationCharacter {
                source_name: "A".into(),
                appearance: "twin tails".into(),
                ..Default::default()
            },
        ]);
        let (system, user) = prompts(&request).unwrap();
        let input: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(input["characters"][0]["appearance"], "twin tails");
        assert!(system.contains("Character sheet"));
        // A matched person is called by `target_name`, not described by looks, and the page's
        // unnamed strangers are not proposed (a proposal without a name is dropped by the parser).
        assert!(system.contains("`target_name`") && system.contains("do not describe their looks"));
        assert!(system.contains("Skip people the page does not name"));
        let schema = request_schema(&request);
        assert_eq!(schema["required"], json!(["description", "characters"]));

        let described = translations("test", r#"{"description":" Two friends. "}"#, &[]).unwrap();
        assert_eq!(described.description.as_deref(), Some("Two friends."));
        // Models send null where the schema says string: that is no description, not an error.
        for response in [r#"{"description":null}"#, r#"{"description":""}"#, "{}"] {
            assert_eq!(
                translations("test", response, &[]).unwrap().description,
                None
            );
        }
        assert!(translations("test", "not json", &[]).is_err());

        let unmatched = translations(
            "test",
            r#"{"description":"x","characters":[{"source_name":"ヒナ","appearance":"red scarf"},{"source_name":"  ","appearance":"b"},{"source_name":"ミト","appearance":null}]}"#,
            &[],
        )
        .unwrap();
        let found: Vec<_> = unmatched
            .proposals
            .characters
            .iter()
            .map(|c| (c.source_name.as_str(), c.appearance.as_str()))
            .collect();
        assert_eq!(found, [("ヒナ", "red scarf"), ("ミト", "")]);
    }

    #[test]
    fn translation_request_carries_the_scene_description() {
        let plain = TranslationRequest::new(["new"], Language::English);
        let (system, user) = prompts(&plain).unwrap();
        assert!(!system.contains("Scene description"));
        assert!(!user.contains("scene_description"));

        let request = plain.with_scene_description(Some("A quarrel at dusk.".into()));
        let (system, user) = prompts(&request).unwrap();
        let input: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(input["scene_description"], "A quarrel at dusk.");
        assert!(system.contains("Scene description"));
        // A blank description is no description.
        let blank = TranslationRequest::new(["new"], Language::English)
            .with_scene_description(Some("  ".into()));
        assert!(blank.scene_description.is_none());
    }

    #[test]
    fn prompt_payload_contains_every_character_and_explains_them() {
        let character = |name: &str| TranslationCharacter {
            source_name: name.into(),
            target_name: format!("{name}-t"),
            speech_style: "curt".into(),
            appearance: "tall".into(),
            notes: "rival".into(),
            ..Default::default()
        };
        let request = TranslationRequest::new(["new"], Language::English)
            .with_characters([character("a"), character("b")]);
        let (system, user) = prompts(&request).unwrap();
        let input: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(input["characters"].as_array().unwrap().len(), 2);
        assert_eq!(input["characters"][1]["target_name"], "b-t");
        assert_eq!(input["characters"][0]["speech_style"], "curt");
        assert!(system.contains("Character sheet"));
        let plain = prompts(&TranslationRequest::new(["new"], Language::English)).unwrap();
        assert!(!plain.0.contains("Character sheet"));
    }

    #[test]
    fn prompt_payload_contains_terms_and_explains_them() {
        let term = |source: &str| TranslationTerm {
            source: source.into(),
            target: format!("{source}-t"),
            note: "title".into(),
            ..Default::default()
        };
        let request =
            TranslationRequest::new(["new"], Language::English).with_terms([term("a"), term("b")]);
        let (system, user) = prompts(&request).unwrap();
        let input: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(input["terms"].as_array().unwrap().len(), 2);
        assert_eq!(input["terms"][1]["target"], "b-t");
        assert_eq!(input["terms"][0]["note"], "title");
        assert!(system.contains("Term list"));
        let plain = prompts(&TranslationRequest::new(["new"], Language::English)).unwrap();
        assert!(!plain.0.contains("Term list"));
    }

    #[test]
    fn prompt_payload_flags_user_corrected_context() {
        let mut line = TranslationContext::new("old", "fixed");
        line.user_corrected = true;
        let request = TranslationRequest::new(["new"], Language::English).with_context([line]);
        let (system, user) = prompts(&request).unwrap();
        let input: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(input["context"][0]["user_corrected"], true);
        assert!(system.contains("user_corrected"));
    }

    #[test]
    fn system_prompt_encodes_invariants_and_custom_instructions() {
        let request = TranslationRequest::new(["hello"], Language::Korean)
            .with_source_language(Language::Japanese)
            .with_instructions("Use informal speech.");
        let prompt = translation_system_prompt(&request);
        assert!(prompt.contains("from Japanese into natural Korean"));
        assert!(prompt.contains("Copy every input ID exactly once"));
        assert!(prompt.contains("Use informal speech."));
    }

    #[test]
    fn schema_requires_the_expected_number_of_id_text_pairs() {
        let schema = output_schema(3);
        let translations = &schema["properties"]["translations"];
        assert_eq!(translations["minItems"], 3);
        assert_eq!(translations["maxItems"], 3);
        assert_eq!(translations["items"]["properties"]["id"]["minimum"], 0);
        assert_eq!(translations["items"]["properties"]["id"]["maximum"], 2);
        assert_eq!(translations["items"]["additionalProperties"], false);
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn schema_accepts_proposals_next_to_translations() {
        let schema = output_schema(1);
        let proposals = &schema["properties"]["proposals"]["properties"];
        assert_eq!(
            proposals["characters"]["items"]["properties"]["source_name"]["type"],
            "string"
        );
        assert_eq!(
            proposals["terms"]["items"]["properties"]["note"]["type"],
            "string"
        );
    }

    #[test]
    fn proposals_are_parsed_entry_by_entry() {
        let source = ["one".to_owned()];
        let response = r#"{"translations":[{"id":0,"text":"hello"}],"proposals":{
            "characters":[{"source_name":"アリス","target_name":"Alice"},{"target_name":"no source"},"junk"],
            "terms":[{"source":"魔王","target":"Demon King","note":"title"},{"source":"  "},{"source":5}]}}"#;
        let translated = translations("test", response, &source).unwrap();
        assert_eq!(translated.segments, ["hello"]);
        assert_eq!(translated.proposals.characters.len(), 1);
        assert_eq!(translated.proposals.characters[0].target_name, "Alice");
        assert_eq!(translated.proposals.terms.len(), 1);
        assert_eq!(translated.proposals.terms[0].source, "魔王");
    }

    #[test]
    fn null_fields_in_a_new_proposal_read_as_empty() {
        // Real model output: `null` where the schema says string.
        let response = r#"{"translations":[{"id":0,"text":"hi"}],"proposals":{"characters":[{"source_name":"ヒナ","target_name":"Hina","speech_style":null,"appearance":null,"notes":"x"}],"terms":[{"source":"a","target":"b","note":null}]}}"#;
        let translated = translations("test", response, &["one".to_owned()]).unwrap();
        assert_eq!(translated.proposals.characters[0].target_name, "Hina");
        assert_eq!(translated.proposals.characters[0].appearance, "");
        assert_eq!(translated.proposals.terms.len(), 1);
    }

    #[test]
    fn updates_keep_only_changed_fields_and_drop_malformed_entries() {
        let source = ["one".to_owned()];
        let response = r#"{"translations":[{"id":0,"text":"hello"}],"proposals":{
            "character_updates":[{"source_name":"アリス","target_name":null,"speech_style":"curt","appearance":null,"notes":null},{"speech_style":"no key"},{"source_name":" "},"junk",{"source_name":"x","notes":5}],
            "term_updates":[{"source":"魔王","target":"Dark Lord","note":null},{"source":7}]}}"#;
        let proposals = translations("test", response, &source).unwrap().proposals;
        assert_eq!(
            proposals.character_updates,
            [TranslationCharacterUpdate {
                source_name: "アリス".into(),
                speech_style: Some("curt".into()),
                ..Default::default()
            }]
        );
        assert_eq!(
            proposals.term_updates,
            [TranslationTermUpdate {
                source: "魔王".into(),
                target: Some("Dark Lord".into()),
                ..Default::default()
            }]
        );
        let schema = output_schema(1);
        let updates = &schema["properties"]["proposals"]["properties"];
        assert_eq!(
            updates["term_updates"]["items"]["properties"]["note"]["type"][1],
            "null"
        );
    }

    #[test]
    fn a_stray_bracket_after_translations_keeps_translations_and_proposals() {
        // Real model output: one `]` too many closes the object early.
        let text = r#"{"translations":[{"id":0,"text":"Hi"},{"id":1,"text":"Umm..."}]],"proposals":{"characters":[],"terms":[{"source":"A","target":"B","note":""}],"character_updates":[],"term_updates":[]}}"#;
        let source = vec!["a".to_owned(), "b".to_owned()];

        let translated = translations("test", text, &source).unwrap();

        assert_eq!(translated.segments, ["Hi", "Umm..."]);
        assert_eq!(translated.proposals.terms.len(), 1);
    }

    #[test]
    fn output_without_translations_still_fails() {
        assert!(translations("test", r#"{"proposals":{}}"#, &["a".to_owned()]).is_err());
    }

    #[test]
    fn missing_empty_or_malformed_proposals_never_fail_the_translation() {
        let source = ["one".to_owned()];
        for proposals in [
            "",
            r#","proposals":null"#,
            r#","proposals":{}"#,
            r#","proposals":{"characters":[],"terms":[]}"#,
            r#","proposals":"nonsense""#,
            r#","proposals":[1,2]"#,
            r#","proposals":{"characters":{"a":1},"terms":7}"#,
        ] {
            let response = format!(r#"{{"translations":[{{"id":0,"text":"hello"}}]{proposals}}}"#);
            let translated = translations("test", &response, &source).unwrap();
            assert_eq!(translated.segments, ["hello"], "{response}");
            assert!(translated.proposals.characters.is_empty(), "{response}");
            assert!(translated.proposals.terms.is_empty(), "{response}");
        }
    }

    #[test]
    fn tentative_entries_are_flagged_and_explained_only_when_present() {
        let request = TranslationRequest::new(["new"], Language::English)
            .with_characters([TranslationCharacter {
                source_name: "a".into(),
                tentative: true,
                ..Default::default()
            }])
            .with_terms([TranslationTerm {
                source: "b".into(),
                ..Default::default()
            }])
            .with_known_sources(["x".to_owned()]);
        let (system, user) = prompts(&request).unwrap();
        let input: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(input["characters"][0]["tentative"], true);
        assert!(input["terms"][0].get("tentative").is_none());
        assert_eq!(input["known_sources"][0], "x");
        assert!(system.contains("tentative"));

        let (system, user) = prompts(&TranslationRequest::new(["new"], Language::English)).unwrap();
        assert!(!system.contains("tentative"));
        assert!(!user.contains("known_sources"));
        assert!(system.contains("Proposal requirements"));
    }

    #[test]
    fn description_prompt_carries_its_instructions() {
        let request = TranslationRequest::describing(Language::English)
            .with_instructions("Name the setting.");
        let prompt = description_system_prompt(&request);
        assert!(prompt.ends_with("Additional instructions:\nName the setting."));
    }

    #[test]
    fn description_request_sends_previous_pages_oldest_first_only_when_given() {
        let plain = TranslationRequest::describing(Language::English);
        let (system, user) = prompts(&plain).unwrap();
        assert!(!system.contains("previous_pages") && !user.contains("previous_pages"));

        let request = plain.with_previous_descriptions([
            PreviousDescription {
                page: 3,
                text: "A quarrel.".into(),
            },
            PreviousDescription {
                page: 4,
                text: "Rain.".into(),
            },
        ]);
        let (system, user) = prompts(&request).unwrap();
        let input: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(input["previous_pages"][0]["page"], 3);
        assert_eq!(input["previous_pages"][1]["text"], "Rain.");
        assert!(system.contains("do not repeat"));
    }

    #[test]
    fn empty_custom_instructions_are_ignored() {
        let request = TranslationRequest::new(["hello"], Language::English).with_instructions("  ");
        assert!(!translation_system_prompt(&request).contains("Additional instructions"));
    }

    #[test]
    fn context_is_reference_only() {
        let request = TranslationRequest::new(["Where is she?"], Language::Japanese)
            .with_context([TranslationContext::new("I saw Alice.", "アリスを見た。")]);
        let prompt = translation_system_prompt(&request);
        assert!(prompt.contains("dialogue continuity"));
        assert!(prompt.contains("Do not translate or return the context"));
    }

    #[test]
    fn image_context_does_not_expand_the_translation_scope() {
        let request = TranslationRequest::new(["text"], Language::English)
            .with_image(std::sync::Arc::new(image::DynamicImage::new_rgb8(1, 1)));
        let prompt = translation_system_prompt(&request);
        assert!(prompt.contains("attached original page image"));
        assert!(prompt.contains("Translate only the supplied segments"));
    }

    #[test]
    fn full_page_request_has_no_note_explanation_or_previous_attempt() {
        let request = TranslationRequest::new(["a"], Language::English);
        let (system, user) = prompts(&request).unwrap();

        assert!(!system.contains("Explanation requirements"));
        assert!(!system.contains("Note requirements"));
        assert!(!user.contains("note") && !user.contains("previous_translation"));
        assert_eq!(request_schema(&request), output_schema(1));
    }

    #[test]
    fn one_box_request_carries_note_previous_attempt_and_explanation() {
        let request = TranslationRequest::new(["イリイラレ"], Language::English)
            .with_explanation(Some("try 入り入られ".into()), Some("Frustration".into()));
        let (system, user) = prompts(&request).unwrap();
        let user: Value = serde_json::from_str(&user).unwrap();

        assert_eq!(user["note"], "try 入り入られ");
        assert_eq!(user["previous_translation"], "Frustration");
        assert!(system.contains("Explanation requirements"));
        assert!(system.contains("Note requirements"));
        let schema = request_schema(&request);
        assert_eq!(schema["properties"]["explanation"]["type"], "string");
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("explanation"))
        );
    }

    #[test]
    fn previous_attempt_is_only_sent_with_a_note() {
        let request = TranslationRequest::new(["a"], Language::English)
            .with_explanation(Some("  ".into()), Some("old".into()));
        let (_, user) = prompts(&request).unwrap();

        assert!(!user.contains("note") && !user.contains("previous_translation"));
        assert!(request.explain);
    }

    #[test]
    fn missing_empty_or_null_explanation_still_applies_the_translation() {
        let source = ["one".to_owned()];
        for tail in [
            "",
            r#","explanation":null"#,
            r#","explanation":"""#,
            r#","explanation":"   ""#,
            r#","explanation":7"#,
        ] {
            let response = format!(r#"{{"translations":[{{"id":0,"text":"hello"}}]{tail}}}"#);
            let translated = translations("test", &response, &source).unwrap();
            assert_eq!(translated.segments, ["hello"], "{tail}");
            assert_eq!(translated.explanation, None, "{tail}");
        }
        let response = r#"{"translations":[{"id":0,"text":"hello"}],"explanation":" why "}"#;
        assert_eq!(
            translations("test", response, &source).unwrap().explanation,
            Some("why".to_owned())
        );
    }
}
