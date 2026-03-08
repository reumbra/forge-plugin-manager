import { useState } from 'react';
import { sendFeedback } from '../lib/api';

const feedbackTypes = [
  { value: 'bug', label: 'Bug Report' },
  { value: 'feature', label: 'Feature Request' },
  { value: 'question', label: 'Question' },
  { value: 'other', label: 'Other' },
];

export default function FeedbackPage() {
  const [type, setType] = useState('bug');
  const [message, setMessage] = useState('');
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async () => {
    if (!message.trim()) return;

    setSending(true);
    setError('');

    try {
      await sendFeedback(type, message.trim());
      setSent(true);
      setMessage('');
    } catch (err) {
      setError(String(err));
    } finally {
      setSending(false);
    }
  };

  if (sent) {
    return (
      <div className="max-w-lg">
        <div className="text-center py-16">
          <div className="w-12 h-12 bg-green-500/10 rounded-full flex items-center justify-center mx-auto mb-4">
            <svg className="w-6 h-6 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
            </svg>
          </div>
          <h3 className="text-lg font-medium text-white mb-2">Thanks for your feedback!</h3>
          <p className="text-sm text-gray-500 mb-6">We'll review it and get back to you if needed.</p>
          <button
            onClick={() => setSent(false)}
            className="px-4 py-2 text-sm text-gray-400 hover:text-white border border-gray-700 hover:border-gray-600 rounded-lg transition-colors"
          >
            Send another
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-lg">
      <h2 className="text-xl font-semibold text-white mb-2">Feedback</h2>
      <p className="text-sm text-gray-500 mb-6">
        Help us improve Forge Plugin Manager.
      </p>

      <div className="space-y-4">
        {/* Type selector */}
        <div>
          <label className="block text-xs text-gray-400 mb-2">Type</label>
          <div className="flex gap-2">
            {feedbackTypes.map((ft) => (
              <button
                key={ft.value}
                onClick={() => setType(ft.value)}
                className={`px-3 py-1.5 text-xs rounded-lg border transition-colors ${
                  type === ft.value
                    ? 'bg-forge-600/20 border-forge-500/40 text-forge-300'
                    : 'border-gray-700 text-gray-500 hover:text-gray-300 hover:border-gray-600'
                }`}
              >
                {ft.label}
              </button>
            ))}
          </div>
        </div>

        {/* Message */}
        <div>
          <label className="block text-xs text-gray-400 mb-2">Message</label>
          <textarea
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            placeholder="Describe your issue or suggestion..."
            rows={6}
            className="w-full px-4 py-3 bg-gray-900 border border-gray-700 rounded-lg text-white text-sm placeholder-gray-600 focus:outline-none focus:border-forge-500 focus:ring-1 focus:ring-forge-500 transition-colors resize-none"
          />
        </div>

        {error && (
          <div className="px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-lg">
            <p className="text-red-400 text-xs">{error}</p>
          </div>
        )}

        <button
          onClick={handleSubmit}
          disabled={sending || !message.trim()}
          className="px-6 py-2.5 bg-forge-600 hover:bg-forge-700 disabled:bg-gray-800 disabled:text-gray-600 text-white text-sm rounded-lg transition-colors"
        >
          {sending ? 'Sending...' : 'Send Feedback'}
        </button>
      </div>
    </div>
  );
}
