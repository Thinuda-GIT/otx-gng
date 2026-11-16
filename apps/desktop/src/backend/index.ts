/** Backend selection. Phase 1 ships only the MockBackend; the TauriBackend (named-pipe client to
 *  the OTX.GNG Core service) is added in Phase 2 and selected automatically when the Core is reachable. */
import { MockBackend } from "./mock";
import type { Backend } from "./types";

export const mockBackend = new MockBackend();
export const backend: Backend = mockBackend;
export const isPreviewData = backend.kind === "mock";

export type * from "./types";
