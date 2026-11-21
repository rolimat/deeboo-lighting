/** Local build flag (`ui/.env`: VITE_DETECTION_DEBUG=true); CI builds never have it. */
export const DEBUG = import.meta.env.VITE_DETECTION_DEBUG === "true";
