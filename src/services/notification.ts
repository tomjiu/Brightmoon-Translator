// Notification Service - 学习提醒服务

import { invoke } from '@tauri-apps/api/core';

export interface NotificationSettings {
  enabled: boolean;
  dailyReminderTime: string; // HH:MM format
  dueCardsThreshold: number;
  milestoneEnabled: boolean;
}

export async function sendDesktopNotification(title: string, body: string): Promise<void> {
  return invoke('send_desktop_notification', { title, body });
}

export async function checkDailyReminder(): Promise<void> {
  return invoke('check_daily_reminder');
}

export async function checkDueCardsReminder(threshold: number): Promise<void> {
  return invoke('check_due_cards_reminder', { threshold });
}

export async function checkMilestoneCelebration(): Promise<void> {
  return invoke('check_milestone_celebration');
}

export async function checkPlanProgressReminder(planId: string): Promise<void> {
  return invoke('check_plan_progress_reminder', { planId });
}
