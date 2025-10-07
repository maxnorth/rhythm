/**
 * Tests for utility functions
 */

import { generateId, getFunctionName } from '../utils.js';

describe('Utils', () => {
  describe('generateId', () => {
    it('should generate ID with prefix', () => {
      const id = generateId('test');
      expect(id).toMatch(/^test_[a-z0-9]+_[a-f0-9]{16}$/);
    });

    it('should generate unique IDs', () => {
      const id1 = generateId('test');
      const id2 = generateId('test');
      expect(id1).not.toBe(id2);
    });
  });

  describe('getFunctionName', () => {
    it('should get function name', () => {
      function testFunction() {}
      expect(getFunctionName(testFunction)).toBe('testFunction');
    });

    it('should return anonymous for unnamed functions', () => {
      const fn = () => {};
      // Arrow functions get inferred names from their variable
      expect(getFunctionName(fn)).toBe('fn');
    });
  });
});
