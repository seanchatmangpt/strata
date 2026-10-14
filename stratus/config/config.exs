import Config

# Ash 3.35 requires an explicit string-length counting policy
# (config :ash, :default_string_length_count). Codepoints matches how
# SQL data layers count, keeping validation consistent end to end.
config :ash, default_string_length_count: :codepoints

# Merger cohort (E-M12) — default OFF. The supervision tree is only
# composed at boot when `:stratus, :merger` has `enabled: true`
# (fail-closed at boot-time composition, mirroring the temprun_worker
# gate in Stratus.Application).
config :stratus, :merger, enabled: false

# Merger temprun lane (E-M12 §6): host bin for the temprun_step bridge
# and the root under which guest artifacts resolve. nil values defer to
# the Worker's own resolution (STRATUS_TEMPRUN_STEP_BIN / documented
# default); set them here to pin the boundary in config.
config :stratus, :merger_temprun, step_bin: nil, guests_root: nil

# FR-4 Ash slice: required by Ash 3.35's RequireStringLengthCountConfig
# transformer (string-length counting semantics for :string attributes).
config :ash, default_string_length_count: :codepoints
