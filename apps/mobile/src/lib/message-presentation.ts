import type { Message } from '@padu/client';

/** Copy exactly what the transcript bubble presents to the user. */
export function messageCopyText(
  message: Pick<Message, 'content' | 'display_content'>,
): string {
  return message.display_content ?? message.content;
}
