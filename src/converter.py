import hashlib
import logging
import math
from pathlib import Path
import struct
import subprocess
import tempfile
import time
from tempfile import SpooledTemporaryFile
from typing import List, BinaryIO, Optional, TypeAlias

import tonie_header_pb2
from src.opus_page import OpusPage
from src.opus_packet import SAMPLE_RATE_KHZ, OpusPacket

HASH: TypeAlias = hashlib._hashlib.HASH

logger = logging.getLogger(__name__)

OPUS_TAGS = [
    bytearray(
        b"\x4f\x70\x75\x73\x54\x61\x67\x73\x0d\x00\x00\x00\x4c\x61\x76\x66\x35\x38\x2e\x32\x30\x2e\x31\x30\x30\x03\x00\x00\x00\x26\x00\x00\x00\x65\x6e\x63\x6f\x64\x65\x72\x3d\x6f\x70\x75\x73\x65\x6e\x63\x20\x66\x72\x6f\x6d\x20\x6f\x70\x75\x73\x2d\x74\x6f\x6f\x6c\x73\x20\x30\x2e\x31\x2e\x31\x30\x2a\x00\x00\x00\x65\x6e\x63\x6f\x64\x65\x72\x5f\x6f\x70\x74\x69\x6f\x6e\x73\x3d\x2d\x2d\x71\x75\x69\x65\x74\x20\x2d\x2d\x62\x69\x74\x72\x61\x74\x65\x20\x39\x36\x20\x2d\x2d\x76\x62\x72\x3b\x01\x00\x00\x70\x61\x64\x3d\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30"
    ),
    bytearray(
        b"\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30\x30"
    ),
]


class Converter:
    def __init__(self):
        pass

    def create_tonie_file(
        self,
        output_file: Path,
        input_files: List[Path],
        no_tonie_header: bool = False,
        user_timestamp: Optional[str] = None,
        bitrate: int = 96,
        cbr: bool = False,
        ffmpeg: str = "ffmpeg",
        opusenc: str = "opusenc",
    ):
        with open(output_file, "wb") as out_file:
            if not no_tonie_header:
                out_file.write(bytearray(0x1000))

            if user_timestamp is not None:
                if user_timestamp.startswith("0x"):
                    timestamp = int(user_timestamp, 16)
                else:
                    timestamp = int(user_timestamp)
            else:
                timestamp = int(time.time())

            sha1 = hashlib.sha1()

            template_page = None
            chapters = []
            total_granule = 0
            next_page_no = 2
            max_size = 0x1000
            other_size = 0xE00
            last_track = False

            pad_len = math.ceil(math.log(len(input_files) + 1, 10))
            format_string = "[{{:0{}d}}/{:0{}d}] {{}}".format(
                pad_len, len(input_files), pad_len
            )

            for index in range(len(input_files)):
                fname = input_files[index]
                print(format_string.format(index + 1, fname))
                if index == len(input_files) - 1:
                    last_track = True

                if fname.suffix == ".opus":
                    handle = open(fname, "rb")
                else:
                    handle = self.get_opus_tempfile(
                        ffmpeg, opusenc, fname, bitrate, not cbr
                    )

                try:
                    if next_page_no == 2:
                        self.copy_first_and_second_page(
                            handle, out_file, timestamp, sha1
                        )
                    else:
                        other_size = max_size
                        self.skip_first_two_pages(handle)

                    pages = self.read_all_remaining_pages(handle)

                    if template_page is None:
                        template_page = OpusPage.from_page(pages[0])
                        template_page.serial_no = timestamp

                    if next_page_no == 2:
                        chapters.append(0)
                    else:
                        chapters.append(next_page_no)

                    new_pages = self.resize_pages(
                        pages,
                        max_size,
                        other_size,
                        template_page,
                        total_granule,
                        next_page_no,
                        last_track,
                    )

                    for new_page in new_pages:
                        new_page.write_page(out_file, sha1)
                    last_page = new_pages[len(new_pages) - 1]
                    total_granule = last_page.granule_position
                    next_page_no = last_page.page_no + 1
                finally:
                    handle.close()

            if not no_tonie_header:
                self.fix_tonie_header(out_file, chapters, timestamp, sha1)

    def fix_tonie_header(
        self, out_file: BinaryIO, chapters: List[int], timestamp: int, sha: HASH
    ):
        tonie_header = tonie_header_pb2.TonieHeader()

        tonie_header.dataHash = sha.digest()
        tonie_header.dataLength = out_file.seek(0, 1) - 0x1000
        tonie_header.timestamp = timestamp

        for chapter in chapters:
            tonie_header.chapterPages.append(chapter)

        tonie_header.padding = bytes(0x100)

        header = tonie_header.SerializeToString()
        pad = 0xFFC - len(header) + 0x100
        tonie_header.padding = bytes(pad)
        header = tonie_header.SerializeToString()

        out_file.seek(0)
        out_file.write(struct.pack(">L", len(header)))
        out_file.write(header)

    def copy_first_and_second_page(
        self, in_file: BinaryIO, out_file: BinaryIO, timestamp: int, sha: HASH
    ):
        found = OpusPage.seek_to_page_header(in_file)
        if not found:
            raise RuntimeError("First ogg page not found")
        page = OpusPage(in_file)
        page.serial_no = timestamp
        page.checksum = page.calc_checksum()
        self.check_identification_header(page)
        page.write_page(out_file, sha)

        found = OpusPage.seek_to_page_header(in_file)
        if not found:
            raise RuntimeError("Second ogg page not found")
        page = OpusPage(in_file)
        page.serial_no = timestamp
        page.checksum = page.calc_checksum()
        page = self.prepare_opus_tags(page)
        page.write_page(out_file, sha)

    def skip_first_two_pages(self, in_file):
        found = OpusPage.seek_to_page_header(in_file)
        if not found:
            raise RuntimeError("First ogg page not found")
        page = OpusPage(in_file)
        self.check_identification_header(page)

        found = OpusPage.seek_to_page_header(in_file)
        if not found:
            raise RuntimeError("Second ogg page not found")
        OpusPage(in_file)

    def read_all_remaining_pages(self, in_file):
        remaining_pages = []

        found = OpusPage.seek_to_page_header(in_file)
        while found:
            remaining_pages.append(OpusPage(in_file))
            found = OpusPage.seek_to_page_header(in_file)
        return remaining_pages

    def resize_pages(
        self,
        old_pages,
        max_page_size,
        first_page_size,
        template_page,
        last_granule=0,
        start_no=2,
        set_last_page_flag=False,
    ):
        new_pages = []
        page = None
        page_no = start_no
        max_size = first_page_size

        new_page = OpusPage.from_page(template_page)
        new_page.page_no = page_no

        while len(old_pages) or (page is not None):
            if page is None:
                page = old_pages.pop(0)

            size = page.get_size_of_first_opus_packet()
            seg_count = page.get_segment_count_of_first_opus_packet()

            if (size + seg_count + new_page.get_page_size() <= max_size) and (
                len(new_page.segments) + seg_count < 256
            ):
                for i in range(seg_count):
                    new_page.segments.append(page.segments.pop(0))
                if not len(page.segments):
                    page = None
            else:
                new_page.pad(max_size)
                new_page.correct_values(last_granule)
                last_granule = new_page.granule_position
                new_pages.append(new_page)

                new_page = OpusPage.from_page(template_page)
                page_no = page_no + 1
                new_page.page_no = page_no
                max_size = max_page_size

        if len(new_page.segments):
            if set_last_page_flag:
                new_page.page_type = 4
            new_page.pad(max_size)
            new_page.correct_values(last_granule)
            new_pages.append(new_page)

        return new_pages

    def prepare_opus_tags(self, page: OpusPage) -> OpusPage:
        page.segments.clear()
        segment = OpusPacket(None)
        segment.size = len(OPUS_TAGS[0])
        segment.data = bytearray(OPUS_TAGS[0])
        segment.spanning_packet = True
        segment.first_packet = True
        page.segments.append(segment)

        segment = OpusPacket(None)
        segment.size = len(OPUS_TAGS[1])
        segment.data = bytearray(OPUS_TAGS[1])
        segment.spanning_packet = False
        segment.first_packet = False
        page.segments.append(segment)
        page.correct_values(0)

        return page

    def check_identification_header(self, page: OpusPage) -> None:
        segment = page.segments[0]
        unpacked = struct.unpack("<8sBBHLH", segment.data[0:18])

        assert unpacked[0] == b"OpusHead", "Invalid opus file?"
        assert unpacked[1] == 1, "Invalid opus file?"
        assert unpacked[2] == 2, "Only stereo tracks are supported"
        assert unpacked[4] == SAMPLE_RATE_KHZ * 1000, "Sample rate needs to be 48 kHz"

    def get_opus_tempfile(
        self,
        ffmpeg_binary: str,
        opus_binary: str,
        filename: Path,
        bitrate: int,
        vbr: bool = True,
    ) -> SpooledTemporaryFile[bytes]:
        if not vbr:
            vbr_parameter = "--hard-cbr"
        else:
            vbr_parameter = "--vbr"

        ffmpeg_process = subprocess.Popen(
            [
                "{}".format(ffmpeg_binary),
                "-hide_banner",
                "-loglevel",
                "warning",
                "-i",
                "{}".format(filename),
                "-f",
                "wav",
                "-ar",
                "48000",
                "-",
            ],
            stdout=subprocess.PIPE,
        )
        opusenc_process = subprocess.Popen(
            [
                "{}".format(opus_binary),
                "--quiet",
                vbr_parameter,
                "--bitrate",
                "{:d}".format(bitrate),
                "-",
                "-",
            ],
            stdin=ffmpeg_process.stdout,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
        )

        tmp_file = tempfile.SpooledTemporaryFile()
        for c in iter(lambda: opusenc_process.stdout.read(1), b""):
            tmp_file.write(c)
        tmp_file.seek(0)

        return tmp_file
