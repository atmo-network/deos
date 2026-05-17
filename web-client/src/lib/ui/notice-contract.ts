/*
Domain: UI Kit notice contract
Owns: Shared prop unions for the Notice primitive and presentation callers.
Excludes: Domain state, widget behavior, and transport/read-model contracts.
Zone: Foundation UI; imported by Notice and callers that return notice state.
*/
export type NoticeVariant = 'muted' | 'warn' | 'success';
