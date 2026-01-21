# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Agorusta is a Discord-like chat application. The project is currently in early setup phase.

## Environment Configuration

The application uses the following services:
- **Backend**: Runs on port 5000
- **Database**: PostgreSQL on port 5432
- **Cache**: Redis on port 6379
- **Frontend**: Next.js connecting to the backend API and WebSocket server

## Architecture Notes

Based on environment configuration, the planned architecture includes:
- JWT-based authentication
- WebSocket support for real-time messaging
- AWS S3 integration for file uploads (planned)
