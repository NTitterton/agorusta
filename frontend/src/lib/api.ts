import { PUBLIC_API_URL, PUBLIC_WS_URL } from '$env/static/public';

export const API_URL = PUBLIC_API_URL;
export const WS_URL = PUBLIC_WS_URL;

// ============ Types ============

export interface User {
	id: string;
	email: string;
	username: string;
}

export interface AuthResponse {
	token: string;
	user: User;
}

export interface Server {
	id: string;
	name: string;
	owner_id: string;
	icon_url: string | null;
	created_at: number;
}

export interface Channel {
	id: string;
	server_id: string;
	name: string;
	channel_type: string;
	created_at: number;
}

export interface Member {
	server_id: string;
	user_id: string;
	username: string;
	role: string;
	joined_at: number;
}

export interface Message {
	id: string;
	channel_id: string;
	author_id: string;
	author_username: string;
	content: string;
	created_at: number;
}

export interface MessagesResponse {
	messages: Message[];
	has_more: boolean;
	next_cursor: number | null;
}

export interface ServerWithChannels extends Server {
	channels: Channel[];
	member_count: number;
}

// ============ Token Management ============

function getToken(): string | null {
	if (typeof window === 'undefined') return null;
	return localStorage.getItem('token');
}

export function setToken(token: string): void {
	localStorage.setItem('token', token);
}

export function clearToken(): void {
	localStorage.removeItem('token');
}

// ============ API Client ============

export async function api<T>(
	endpoint: string,
	options: RequestInit = {}
): Promise<{ data?: T; error?: string }> {
	try {
		const token = getToken();
		const headers: Record<string, string> = {
			'Content-Type': 'application/json',
			...(options.headers as Record<string, string>)
		};

		if (token) {
			headers['Authorization'] = `Bearer ${token}`;
		}

		const response = await fetch(`${API_URL}${endpoint}`, {
			...options,
			headers
		});

		const data = await response.json();

		if (!response.ok) {
			return { error: data.error || 'Request failed' };
		}

		return { data };
	} catch (err) {
		return { error: err instanceof Error ? err.message : 'Network error' };
	}
}

// ============ Health ============

export async function checkHealth(): Promise<{ ok: boolean; error?: string }> {
	const result = await api<{ status: string }>('/health');
	if (result.error) {
		return { ok: false, error: result.error };
	}
	return { ok: result.data?.status === 'ok' };
}

// ============ Auth ============

export async function register(
	email: string,
	username: string,
	password: string
): Promise<{ data?: AuthResponse; error?: string }> {
	return api<AuthResponse>('/auth/register', {
		method: 'POST',
		body: JSON.stringify({ email, username, password })
	});
}

export async function login(
	email: string,
	password: string
): Promise<{ data?: AuthResponse; error?: string }> {
	return api<AuthResponse>('/auth/login', {
		method: 'POST',
		body: JSON.stringify({ email, password })
	});
}

export async function getMe(): Promise<{ data?: User; error?: string }> {
	return api<User>('/auth/me');
}

// ============ Servers ============

export async function getServers(): Promise<{ data?: Server[]; error?: string }> {
	return api<Server[]>('/servers');
}

export async function getServer(serverId: string): Promise<{ data?: ServerWithChannels; error?: string }> {
	return api<ServerWithChannels>(`/servers/${serverId}`);
}

export async function createServer(name: string): Promise<{ data?: ServerWithChannels; error?: string }> {
	return api<ServerWithChannels>('/servers', {
		method: 'POST',
		body: JSON.stringify({ name })
	});
}

// ============ Channels ============

export async function getChannels(serverId: string): Promise<{ data?: Channel[]; error?: string }> {
	return api<Channel[]>(`/servers/${serverId}/channels`);
}

export async function createChannel(
	serverId: string,
	name: string,
	channelType: string = 'text'
): Promise<{ data?: Channel; error?: string }> {
	return api<Channel>(`/servers/${serverId}/channels`, {
		method: 'POST',
		body: JSON.stringify({ name, channel_type: channelType })
	});
}

// ============ Members ============

export async function getMembers(serverId: string): Promise<{ data?: Member[]; error?: string }> {
	return api<Member[]>(`/servers/${serverId}/members`);
}

// ============ Messages ============

export async function getMessages(
	serverId: string,
	channelId: string,
	options?: { limit?: number; before?: number }
): Promise<{ data?: MessagesResponse; error?: string }> {
	const params = new URLSearchParams();
	if (options?.limit) params.set('limit', options.limit.toString());
	if (options?.before) params.set('before', options.before.toString());
	const query = params.toString() ? `?${params}` : '';
	return api<MessagesResponse>(`/servers/${serverId}/channels/${channelId}/messages${query}`);
}

export async function sendMessage(
	serverId: string,
	channelId: string,
	content: string
): Promise<{ data?: Message; error?: string }> {
	return api<Message>(`/servers/${serverId}/channels/${channelId}/messages`, {
		method: 'POST',
		body: JSON.stringify({ content })
	});
}
