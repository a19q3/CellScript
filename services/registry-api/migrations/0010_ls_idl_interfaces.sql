create index package_version_evidence_ls_idl_lookup_idx
  on package_version_evidence (
    lower(regexp_replace(evidence->>'code_hash', '^0x', '', 'i')),
    (evidence->>'network'),
    (evidence->>'hash_type'),
    lower(regexp_replace(evidence->>'data_hash', '^0x', '', 'i')),
    created_at desc
  )
  where kind = 'deployed';
