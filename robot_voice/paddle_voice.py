import re

import cn2an
import numpy as np
import onnxruntime as ort
from pypinyin import Style
from pypinyin.core import Pinyin
from pypinyin.contrib.tone_sandhi import ToneSandhiMixin
from pypinyin.converter import DefaultConverter


class MandarinConverter(ToneSandhiMixin, DefaultConverter):
    pass


class PaddleVoice:
    """PaddleSpeech CSMSC 的声学模型与声码器均使用 24 kHz。"""

    def __init__(self, models):
        acoustic = models / "fastspeech2_csmsc_onnx_0.2.0"
        vocoder = models / "hifigan_csmsc_onnx_0.2.0"
        opts = ort.SessionOptions()
        opts.intra_op_num_threads = 3
        self.acoustic = ort.InferenceSession(str(acoustic / "fastspeech2_csmsc.onnx"), opts)
        self.vocoder = ort.InferenceSession(str(vocoder / "hifigan_csmsc.onnx"), opts)
        entries = (acoustic / "phone_id_map.txt").read_text().splitlines()
        self.ids = {phone: int(index) for phone, index in (line.split() for line in entries)}
        self.pinyin = Pinyin(MandarinConverter())

    def generate(self, text):
        # CSMSC 的音素表只覆盖普通话。交给调用方处理包含外语的句子。
        if re.search(r"[A-Za-z]", text):
            return None
        text = cn2an.transform(text, "an2cn")
        options = dict(neutral_tone_with_five=True, errors=lambda chars: ["sp"])
        initials = self.pinyin.lazy_pinyin(text, style=Style.INITIALS, **options)
        finals = self.pinyin.lazy_pinyin(text, style=Style.FINALS_TONE3, **options)
        phones = []
        for initial, final in zip(initials, finals):
            if final == "sp":
                if phones and phones[-1] != "sp":
                    phones.append("sp")
                continue
            if final and final[-1] not in "12345":
                final += "5"
            if re.fullmatch(r"i[1-5]", final):
                if initial in ("z", "c", "s"):
                    final = "i" + final
                elif initial in ("zh", "ch", "sh", "r"):
                    final = "ii" + final
            phones.extend(p for p in (initial, final) if p)
        if not phones or any(p not in self.ids for p in phones):
            return None
        ids = np.asarray([self.ids[p] for p in phones], dtype=np.int64)
        mel, = self.acoustic.run(None, {"text": ids})
        samples, = self.vocoder.run(None, {"logmel": mel})
        return samples.ravel()
