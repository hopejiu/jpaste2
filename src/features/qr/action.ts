import { register } from '../../actions/registry';
import { api } from '../../lib/invoke';
import { copyToClipboard } from '../../lib/clipboard';
import { debug } from '../../lib/logger';
import { TAG_QR } from '../../lib/types';

/**
 * QR Code action — for image entries that contain a QR code.
 *
 * Differs from other actions:
 * - detect() checks tag_mask (not content text)
 * - handler() copies the QR text to clipboard on click
 */
register({
  id: 'qrcode',
  label: '扫描二维码',
  priority: 100, // Highest priority — QR is an explicit image-based action
  detect(_content: string, tagMask?: number): boolean {
    // ponytail: first arg is unused for QR detection; detection relies on
    // tag_mask. Upgrade path: pass entry object if more image-based actions.
    if (tagMask !== undefined) {
      return (tagMask & TAG_QR) !== 0;
    }
    return false;
  },
  handler(_content: string, entryId: number) {
    api.scanQrText(entryId)
      .then((qrText) => {
        if (qrText) {
          copyToClipboard(qrText);
          debug('qrcode: copied to clipboard', qrText);
        }
      })
      .catch((err) => debug('qrcode: scan failed', err));
  },
});
