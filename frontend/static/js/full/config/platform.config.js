/**
 * Platform Config
 * Contains environment-specific settings.
 */

export const PlatformConfig = {
    // Set to 'full' for development (debuggable source files), 'min' for production (minified files).
    // In production, this can be injected by the build system.
    ENV: "min",
};

window.PlatformConfig = PlatformConfig;
export default PlatformConfig;
