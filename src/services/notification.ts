// Notification Service - 学习提醒服务

import { invokeOrThrow } from './invoke';

export interface NotificationSettings {
  enabled: boolean;
  dailyReminderTime: string; // HH:MM format
  dueCardsThreshold: number;
  milestoneEnabled: boolean;
}

export async function sendDesktopNotification(title: string, body: string): Promise<void> {
  return invokeOrThrow('send_desktop_notification', { title, body });
}

export async function checkDailyReminder(): Promise<void> {
  return invokeOrThrow('check_daily_reminder');
}

export async function checkDueCardsReminder(threshold: number): Promise<void> {
  return invokeOrThrow('check_due_cards_reminder', { threshold });
}

export async function checkMilestoneCelebration(): Promise<void> {
  return invokeOrThrow('check_milestone_celebration');
}

export async function checkPlanProgressReminder(planId: string): Promise<void> {
  return invokeOrThrow('check_plan_progress_reminder', { planId });
}
