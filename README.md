# TraceBoost

TraceBoost is a Tauri desktop application for seismic data upscaling.

The initial product direction is local-first upscaling of rasterized seismic sections and slices using learned super-resolution. That means the application will focus first on reconstructing higher-resolution visual output from lower-resolution seismic imagery, rather than treating upscaling as bit-depth conversion, trace interpolation, or SEG-Y resampling.

## Initial Scope

- build a Tauri desktop application
- support seismic image and slice workflows
- use learned super-resolution as the first upscaling method
- keep the application local-first
- use the current `docs/` directory as the planning and architecture baseline

## Notes

- the `docs/` directory currently contains research notes, architecture analysis, and imported reference material that we will refine as the product direction solidifies
- the implementation target is a seismic-focused desktop application, not a general image tool
