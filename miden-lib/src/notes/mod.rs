use crate::assembler::assembler;
use crate::memory::{
    CREATED_NOTE_ASSETS_OFFSET, CREATED_NOTE_CORE_DATA_SIZE, CREATED_NOTE_HASH_OFFSET,
    CREATED_NOTE_METADATA_OFFSET, CREATED_NOTE_RECIPIENT_OFFSET, CREATED_NOTE_VAULT_HASH_OFFSET,
};
use miden_objects::{
    accounts::AccountId,
    assembly::ProgramAst,
    assets::Asset,
    notes::{Note, NoteMetadata, NoteScript, NoteStub, NoteVault},
    utils::{collections::Vec, format, vec},
    Digest, Felt, NoteError, StarkField, Word, WORD_SIZE, ZERO,
};

pub enum Script {
    P2ID {
        target: AccountId,
    },
    P2IDR {
        target: AccountId,
        recall_height: u32,
    },
}

/// Users can create notes with a standard script. Atm we provide two standard scripts:
/// 1. P2ID - pay to id
/// 2. P2IDR - pay to id with recall after a certain block height
pub fn create_note(
    script: Script,
    assets: Vec<Asset>,
    sender: AccountId,
    tag: Option<Felt>,
    serial_num: Word,
) -> Result<Note, NoteError> {
    let note_assembler = assembler();
    let (note_script, inputs): (&str, Vec<Felt>) = match script {
        Script::P2ID { target } => ("p2id", vec![target.into()]), // Convert `target` to a suitable type if necessary
        Script::P2IDR {
            target,
            recall_height,
        } => ("p2idr", vec![target.into(), recall_height.into()]), // Convert both to a suitable type
    };

    // Create the note
    let note_script_str = format!(
        "
        use.miden::note_scripts::basic
    
        begin
            exec.basic::{note_script}
        end
        "
    );

    let note_script_ast = ProgramAst::parse(&note_script_str)
        .map_err(|e| NoteError::ScriptCompilationError(e.into()))?;

    let (note_script, _) = NoteScript::new(note_script_ast, &note_assembler)?;

    Note::new(
        note_script.clone(),
        &inputs,
        &assets,
        serial_num,
        sender,
        tag.unwrap_or(ZERO),
        None,
    )
}

pub fn notes_try_from_elements(elements: &[Word]) -> Result<NoteStub, NoteError> {
    if elements.len() < CREATED_NOTE_CORE_DATA_SIZE {
        return Err(NoteError::InvalidStubDataLen(elements.len()));
    }

    let hash: Digest = elements[CREATED_NOTE_HASH_OFFSET as usize].into();
    let metadata: NoteMetadata = elements[CREATED_NOTE_METADATA_OFFSET as usize].try_into()?;
    let recipient = elements[CREATED_NOTE_RECIPIENT_OFFSET as usize].into();
    let vault_hash: Digest = elements[CREATED_NOTE_VAULT_HASH_OFFSET as usize].into();

    if elements.len()
        < (CREATED_NOTE_ASSETS_OFFSET as usize + metadata.num_assets().as_int() as usize)
            * WORD_SIZE
    {
        return Err(NoteError::InvalidStubDataLen(elements.len()));
    }

    let vault: NoteVault = elements[CREATED_NOTE_ASSETS_OFFSET as usize
        ..(CREATED_NOTE_ASSETS_OFFSET as usize + metadata.num_assets().as_int() as usize)]
        .try_into()?;
    if vault.hash() != vault_hash {
        return Err(NoteError::InconsistentStubVaultHash(vault_hash, vault.hash()));
    }

    let stub = NoteStub::new(recipient, vault, metadata)?;
    if stub.hash() != hash {
        return Err(NoteError::InconsistentStubHash(stub.hash(), hash));
    }

    Ok(stub)
}