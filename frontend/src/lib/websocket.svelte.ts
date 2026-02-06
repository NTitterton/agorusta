import { WS_URL, type Message, type DirectMessage } from './api';

type MessageHandler = (message: Message) => void;
type DmHandler = (message: DirectMessage) => void;
type TypingHandler = (typingUsers: Map<string, string>) => void; // user_id -> username

interface TypingUser {
	username: string;
	timeout: ReturnType<typeof setTimeout>;
}

class WebSocketService {
	private ws: WebSocket | null = null;
	private reconnectAttempts = 0;
	private maxReconnectAttempts = 5;
	private reconnectDelay = 1000;
	private messageHandlers: Map<string, Set<MessageHandler>> = new Map();
	private dmHandlers: Map<string, Set<DmHandler>> = new Map();
	private typingHandlers: Map<string, Set<TypingHandler>> = new Map();
	private typingUsers: Map<string, Map<string, TypingUser>> = new Map(); // channel_id -> (user_id -> TypingUser)
	private subscribedChannels: Set<string> = new Set();

	connected = $state(false);
	error = $state<string | null>(null);

	connect() {
		if (this.ws?.readyState === WebSocket.OPEN) {
			return; // Already connected
		}

		const token = typeof window !== 'undefined' ? localStorage.getItem('token') : null;
		if (!token) {
			this.error = 'Not authenticated';
			return;
		}

		try {
			this.ws = new WebSocket(`${WS_URL}?token=${token}`);

			this.ws.onopen = () => {
				this.connected = true;
				this.error = null;
				this.reconnectAttempts = 0;
				console.log('WebSocket connected');

				// Re-subscribe to previously subscribed channels
				this.subscribedChannels.forEach((channelId) => {
					this.sendSubscribe(channelId);
				});
			};

			this.ws.onclose = (event) => {
				this.connected = false;
				console.log('WebSocket disconnected', event.code, event.reason);

				// Attempt reconnection if not a clean close
				if (!event.wasClean && this.reconnectAttempts < this.maxReconnectAttempts) {
					const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts);
					console.log(`Reconnecting in ${delay}ms...`);
					setTimeout(() => {
						this.reconnectAttempts++;
						this.connect();
					}, delay);
				}
			};

			this.ws.onerror = (event) => {
				console.error('WebSocket error:', event);
				this.error = 'WebSocket connection error';
			};

			this.ws.onmessage = (event) => {
				try {
					const data = JSON.parse(event.data);
					if (data.type === 'new_message') {
						const message = data.message as Message;
						const handlers = this.messageHandlers.get(message.channel_id);
						handlers?.forEach((handler) => handler(message));
					} else if (data.type === 'new_dm') {
						const message = data.message as DirectMessage;
						const handlers = this.dmHandlers.get(message.conversation_id);
						handlers?.forEach((handler) => handler(message));
					} else if (data.type === 'user_typing') {
						this.handleTypingEvent(data.channel_id, data.user_id, data.username, true);
					} else if (data.type === 'user_stop_typing') {
						this.handleTypingEvent(data.channel_id, data.user_id, data.username, false);
					}
				} catch (e) {
					console.error('Failed to parse WebSocket message:', e);
				}
			};
		} catch (e) {
			console.error('Failed to create WebSocket:', e);
			this.error = 'Failed to connect';
		}
	}

	private handleTypingEvent(channelId: string, userId: string, username: string, isTyping: boolean) {
		if (!this.typingUsers.has(channelId)) {
			this.typingUsers.set(channelId, new Map());
		}
		const channelTyping = this.typingUsers.get(channelId)!;

		if (isTyping) {
			// Clear existing timeout if any
			const existing = channelTyping.get(userId);
			if (existing) {
				clearTimeout(existing.timeout);
			}

			// Set new timeout to auto-remove after 5 seconds
			const timeout = setTimeout(() => {
				channelTyping.delete(userId);
				this.notifyTypingHandlers(channelId);
			}, 5000);

			channelTyping.set(userId, { username, timeout });
		} else {
			// Remove user from typing list
			const existing = channelTyping.get(userId);
			if (existing) {
				clearTimeout(existing.timeout);
				channelTyping.delete(userId);
			}
		}

		this.notifyTypingHandlers(channelId);
	}

	private notifyTypingHandlers(channelId: string) {
		const handlers = this.typingHandlers.get(channelId);
		if (!handlers) return;

		const channelTyping = this.typingUsers.get(channelId);
		const typingMap = new Map<string, string>();
		if (channelTyping) {
			channelTyping.forEach((user, id) => typingMap.set(id, user.username));
		}

		handlers.forEach((handler) => handler(typingMap));
	}

	disconnect() {
		if (this.ws) {
			this.ws.close(1000, 'User disconnected');
			this.ws = null;
		}
		this.connected = false;
		this.subscribedChannels.clear();
		this.messageHandlers.clear();
		this.dmHandlers.clear();
		this.typingHandlers.clear();
		// Clear all typing timeouts
		this.typingUsers.forEach((channelTyping) => {
			channelTyping.forEach((user) => clearTimeout(user.timeout));
		});
		this.typingUsers.clear();
	}

	subscribeToChannel(channelId: string, handler: MessageHandler): () => void {
		// Track handler
		if (!this.messageHandlers.has(channelId)) {
			this.messageHandlers.set(channelId, new Set());
		}
		this.messageHandlers.get(channelId)!.add(handler);

		// Send subscribe if not already subscribed
		if (!this.subscribedChannels.has(channelId)) {
			this.subscribedChannels.add(channelId);
			this.sendSubscribe(channelId);
		}

		// Return unsubscribe function
		return () => {
			this.messageHandlers.get(channelId)?.delete(handler);
			if (this.messageHandlers.get(channelId)?.size === 0) {
				this.messageHandlers.delete(channelId);
				this.subscribedChannels.delete(channelId);
				this.sendUnsubscribe(channelId);
			}
		};
	}

	private sendSubscribe(channelId: string) {
		if (this.ws?.readyState === WebSocket.OPEN) {
			this.ws.send(
				JSON.stringify({
					action: 'subscribe',
					channel_id: channelId
				})
			);
			console.log('Subscribed to channel:', channelId);
		}
	}

	private sendUnsubscribe(channelId: string) {
		if (this.ws?.readyState === WebSocket.OPEN) {
			this.ws.send(
				JSON.stringify({
					action: 'unsubscribe',
					channel_id: channelId
				})
			);
			console.log('Unsubscribed from channel:', channelId);
		}
	}

	subscribeToConversation(conversationId: string, handler: DmHandler): () => void {
		// Track handler
		if (!this.dmHandlers.has(conversationId)) {
			this.dmHandlers.set(conversationId, new Set());
		}
		this.dmHandlers.get(conversationId)!.add(handler);

		// Send subscribe if not already subscribed (uses same mechanism as channels)
		if (!this.subscribedChannels.has(conversationId)) {
			this.subscribedChannels.add(conversationId);
			this.sendSubscribe(conversationId);
		}

		// Return unsubscribe function
		return () => {
			this.dmHandlers.get(conversationId)?.delete(handler);
			if (this.dmHandlers.get(conversationId)?.size === 0) {
				this.dmHandlers.delete(conversationId);
				this.subscribedChannels.delete(conversationId);
				this.sendUnsubscribe(conversationId);
			}
		};
	}

	subscribeToTyping(channelId: string, handler: TypingHandler): () => void {
		if (!this.typingHandlers.has(channelId)) {
			this.typingHandlers.set(channelId, new Set());
		}
		this.typingHandlers.get(channelId)!.add(handler);

		// Return unsubscribe function
		return () => {
			this.typingHandlers.get(channelId)?.delete(handler);
			if (this.typingHandlers.get(channelId)?.size === 0) {
				this.typingHandlers.delete(channelId);
			}
		};
	}

	sendTyping(channelId: string) {
		if (this.ws?.readyState === WebSocket.OPEN) {
			this.ws.send(
				JSON.stringify({
					action: 'typing',
					channel_id: channelId
				})
			);
		}
	}

	sendStopTyping(channelId: string) {
		if (this.ws?.readyState === WebSocket.OPEN) {
			this.ws.send(
				JSON.stringify({
					action: 'stop_typing',
					channel_id: channelId
				})
			);
		}
	}
}

export const websocket = new WebSocketService();
