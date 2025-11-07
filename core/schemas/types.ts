/**
 * Core TypeScript type definitions for Personal Memory Layer
 * Generated from JSON schemas
 */

// ============================================================================
// ID Types
// ============================================================================

export type TurnId = string; // "turn_{ULID}"
export type ThreadId = string; // "thr_{ULID}"
export type MemoryId = string; // "mem_{ULID}"
export type CapsuleId = string; // "cap_{ULID}"

// ============================================================================
// Turn Schema
// ============================================================================

export interface Turn {
  id: TurnId;
  thread_id: ThreadId;
  ts_user: string; // RFC3339
  user_text: string;
  ts_ai?: string; // RFC3339
  ai_text?: string;
  source: TurnSource;
}

export interface TurnSource {
  app: SourceApp;
  url?: string;
  path?: string;
}

export type SourceApp =
  | 'Claude'
  | 'ChatGPT'
  | 'VSCode'
  | 'Mail'
  | 'Notes'
  | 'Terminal'
  | 'Other';

// ============================================================================
// Memory Schema
// ============================================================================

export interface Memory {
  id: MemoryId;
  kind: MemoryKind;
  topic: string;
  text: string;
  snippet?: Snippet;
  entities: string[];
  provenance: TurnId[];
  created_at: string; // RFC3339
  ttl: number | null;
}

export type MemoryKind = 'decision' | 'fact' | 'snippet' | 'task';

export interface Snippet {
  title: string;
  text: string;
  loc?: string; // e.g., "L18-L44"
  language?: string;
}

// ============================================================================
// Context Capsule Schema
// ============================================================================

export interface ContextCapsule {
  capsule_id: CapsuleId;
  preamble_text: string;
  messages: Message[];
  provenance: ProvenanceItem[];
  delta_of?: CapsuleId;
  ttl_sec: number;
  token_count?: number;
  style?: ContextStyle;
}

export interface Message {
  role: MessageRole;
  content: string;
}

export type MessageRole = 'system' | 'user' | 'assistant';

export interface ProvenanceItem {
  type: ProvenanceType;
  ref: string;
  when?: string; // RFC3339
}

export type ProvenanceType =
  | 'assistant'
  | 'file'
  | 'page'
  | 'terminal'
  | 'memory';

export type ContextStyle = 'short' | 'standard' | 'detailed';

// ============================================================================
// API Request/Response Types
// ============================================================================

export interface ContextRequest {
  topic_hint?: string;
  intent?: string;
  budget_tokens: number;
  scopes: string[];
  thread_key?: string;
  last_capsule_id?: CapsuleId;
}

export interface UndoRequest {
  capsule_id: CapsuleId;
  thread_key: string;
}

export interface UndoResponse {
  success: boolean;
  message?: string;
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Generate a new ULID-based ID with the given prefix
 */
export function generateId(prefix: string): string {
  // Simple ULID generation (in real impl, use a proper ULID library)
  const timestamp = Date.now();
  const randomness = Math.random().toString(36).substring(2, 15) +
                     Math.random().toString(36).substring(2, 15);
  return `${prefix}_${timestamp}${randomness}`.substring(0, 31);
}

export function generateTurnId(): TurnId {
  return generateId('turn');
}

export function generateThreadId(): ThreadId {
  return generateId('thr');
}

export function generateMemoryId(): MemoryId {
  return generateId('mem');
}

export function generateCapsuleId(): CapsuleId {
  return generateId('cap');
}

/**
 * Validate ID format
 */
export function isValidId(id: string, prefix: string): boolean {
  const pattern = new RegExp(`^${prefix}_[0-9A-HJKMNP-TV-Z]{26}$`);
  return pattern.test(id);
}

export function isValidTurnId(id: string): boolean {
  return isValidId(id, 'turn');
}

export function isValidThreadId(id: string): boolean {
  return isValidId(id, 'thr');
}

export function isValidMemoryId(id: string): boolean {
  return isValidId(id, 'mem');
}

export function isValidCapsuleId(id: string): boolean {
  return isValidId(id, 'cap');
}

// ============================================================================
// Type Guards
// ============================================================================

export function isTurn(obj: any): obj is Turn {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    typeof obj.id === 'string' &&
    typeof obj.thread_id === 'string' &&
    typeof obj.ts_user === 'string' &&
    typeof obj.user_text === 'string' &&
    typeof obj.source === 'object'
  );
}

export function isMemory(obj: any): obj is Memory {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    typeof obj.id === 'string' &&
    typeof obj.kind === 'string' &&
    typeof obj.topic === 'string' &&
    typeof obj.text === 'string' &&
    Array.isArray(obj.entities) &&
    Array.isArray(obj.provenance)
  );
}

export function isContextCapsule(obj: any): obj is ContextCapsule {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    typeof obj.capsule_id === 'string' &&
    typeof obj.preamble_text === 'string' &&
    Array.isArray(obj.messages) &&
    Array.isArray(obj.provenance) &&
    typeof obj.ttl_sec === 'number'
  );
}
