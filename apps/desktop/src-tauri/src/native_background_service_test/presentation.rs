// Exact original pure presentation algorithm; source-inverse pinned by tests.
use crate::native_parent::{REVIEW_MESSAGE_BYTES, REVIEW_ROWS_PER_PAGE, REVIEW_ROW_ENCODED_BYTES};
pub(crate) fn review_prompts(message: &str) -> Result<Vec<String>, String> {
    if message.len() > REVIEW_MESSAGE_BYTES {
        return Err("Complete native message exceeds the bounded presentation envelope".into());
    }
    let mut rows = Vec::new();
    let mut encoded = String::new();
    let mut start = 0;
    let mut end = 0;
    for (offset, ch) in message.char_indices() {
        let escaped = if ch == '\u{061c}'
            || ch == '\x7f'
            || ('\u{0080}'..='\u{009f}').contains(&ch)
            || ('\u{200b}'..='\u{200f}').contains(&ch)
            || ('\u{2028}'..='\u{202e}').contains(&ch)
            || ('\u{2060}'..='\u{206f}').contains(&ch)
            || ch == '\u{feff}'
        {
            format!("\\u{:04x}", ch as u32)
        } else {
            let quoted = serde_json::to_string(&ch.to_string()).map_err(|e| e.to_string())?;
            quoted[1..quoted.len() - 1].to_owned()
        };
        if !encoded.is_empty() && encoded.len() + escaped.len() > REVIEW_ROW_ENCODED_BYTES {
            rows.push((start, offset, std::mem::take(&mut encoded)));
            start = offset;
        }
        encoded.push_str(&escaped);
        end = offset + ch.len_utf8();
    }
    if !encoded.is_empty() || rows.is_empty() {
        rows.push((start, end, encoded));
    }
    // Verify the exact roundtrip before opening even the first native page.
    let mut restored = String::new();
    for (start, end, encoded) in &rows {
        let decoded: String = serde_json::from_str(&format!("\"{encoded}\""))
            .map_err(|_| "Native review encoding could not be restored")?;
        if message.get(*start..*end) != Some(decoded.as_str()) {
            return Err("Native review encoding changed original bytes".into());
        }
        restored.push_str(&decoded);
    }
    if restored != message {
        return Err("Native review encoding omitted original bytes".into());
    }
    let total = rows.len().div_ceil(REVIEW_ROWS_PER_PAGE);
    let mut prompts = Vec::with_capacity(total + 1);
    for (index, page) in rows.chunks(REVIEW_ROWS_PER_PAGE).enumerate() {
        let body = page
            .iter()
            .enumerate()
            .map(|(row, (_, _, text))| {
                format!(
                    "L{:05} \"{}\"",
                    index * REVIEW_ROWS_PER_PAGE + row + 1,
                    text
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        prompts.push(format!("Native review page {}/{}\nOriginal UTF-8 bytes {}..{} of {}.\nRead JSON strings in L order; escapes preserve ALL text.\nOK reads this page only. Cancel stops this review.\n{}", index + 1, total, page[0].0, page.last().unwrap().1, message.len(), body));
    }
    prompts.push(format!("FINAL display confirmation\nAll {} pages / {} original UTF-8 bytes were reviewed.\nOK confirms the original request exactly as displayed.\nAny further required reviews still apply.\nCancel stops without private approval.", total, message.len()));
    Ok(prompts)
}
