import assert from 'node:assert/strict'
import { $, browser } from '@wdio/globals'

export async function byTestId(testId: string) {
  return $(`[data-testid="${testId}"]`)
}

export async function waitForTestId(testId: string, timeout = 30000) {
  const element = await byTestId(testId)
  await element.waitForExist({ timeout })
  return element
}

export async function completeWelcomeIfPresent() {
  const welcomeEmpty = await byTestId('welcome-empty-layout')
  const exists = await welcomeEmpty.waitForExist({
    timeout: 1500,
    reverse: false,
  }).catch(() => false)
  if (!exists) return

  await welcomeEmpty.click()
  await waitForTestId('settings-page', 30000)
}

export async function openHome() {
  await completeWelcomeIfPresent()
  const home = await waitForTestId('home-nav-button')
  await home.click()
  await waitForTestId('layouts-page')
}

export async function openSettings() {
  await completeWelcomeIfPresent()
  const settings = await waitForTestId('settings-nav-button')
  await settings.click()
  await waitForTestId('settings-page')
}

export async function waitForAttribute(
  testId: string,
  name: string,
  predicate: (value: string) => boolean,
  timeout = 30000,
) {
  const element = await waitForTestId(testId)
  await browser.waitUntil(
    async () => {
      const value = await element.getAttribute(name)
      return value != null && predicate(value)
    },
    {
      timeout,
      timeoutMsg: `Timed out waiting for ${testId} ${name}`,
    },
  )
  return await element.getAttribute(name)
}

export function normalizePathForAssert(value: string) {
  return value.replaceAll('\\', '/')
}

export function assertContainsPath(actual: string, expected: string) {
  assert.ok(
    normalizePathForAssert(actual).includes(normalizePathForAssert(expected)),
    `Expected "${actual}" to contain "${expected}"`,
  )
}
