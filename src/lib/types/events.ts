export interface ModelStateEvent {
  event_type: string;
  model_id?: string;
  model_name?: string;
  error?: string;
}

export interface RecordingErrorEvent {
  error_type: string;
  detail?: string;
}

export interface TranscriptionErrorEvent {
  message: string;
  is_network_error: boolean;

  audio_saved: boolean;

  notice: string | null;
}

export interface AudioTooLongEvent {
  audio_saved: boolean;
}
