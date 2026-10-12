import Config

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
