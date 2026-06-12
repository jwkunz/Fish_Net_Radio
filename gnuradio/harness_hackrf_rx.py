#!/usr/bin/env python3
# -*- coding: utf-8 -*-

#
# SPDX-License-Identifier: GPL-3.0
#
# GNU Radio Python Flow Graph
# Title: Harness of HackRF RX
# Author: Jakob Kunzler
# GNU Radio version: 3.10.12.0

from PyQt5 import Qt
from gnuradio import qtgui
from PyQt5 import QtCore
from gnuradio import blocks
from gnuradio import gr
from gnuradio.filter import firdes
from gnuradio.fft import window
import sys
import signal
from PyQt5 import Qt
from argparse import ArgumentParser
from gnuradio.eng_arg import eng_float, intx
from gnuradio import eng_notation
from gnuradio import soapy
from gnuradio import zeromq
import sip
import threading



class harness_hackrf_rx(gr.top_block, Qt.QWidget):

    def __init__(self):
        gr.top_block.__init__(self, "Harness of HackRF RX", catch_exceptions=True)
        Qt.QWidget.__init__(self)
        self.setWindowTitle("Harness of HackRF RX")
        qtgui.util.check_set_qss()
        try:
            self.setWindowIcon(Qt.QIcon.fromTheme('gnuradio-grc'))
        except BaseException as exc:
            print(f"Qt GUI: Could not set Icon: {str(exc)}", file=sys.stderr)
        self.top_scroll_layout = Qt.QVBoxLayout()
        self.setLayout(self.top_scroll_layout)
        self.top_scroll = Qt.QScrollArea()
        self.top_scroll.setFrameStyle(Qt.QFrame.NoFrame)
        self.top_scroll_layout.addWidget(self.top_scroll)
        self.top_scroll.setWidgetResizable(True)
        self.top_widget = Qt.QWidget()
        self.top_scroll.setWidget(self.top_widget)
        self.top_layout = Qt.QVBoxLayout(self.top_widget)
        self.top_grid_layout = Qt.QGridLayout()
        self.top_layout.addLayout(self.top_grid_layout)

        self.settings = Qt.QSettings("gnuradio/flowgraphs", "harness_hackrf_rx")

        try:
            geometry = self.settings.value("geometry")
            if geometry:
                self.restoreGeometry(geometry)
        except BaseException as exc:
            print(f"Qt GUI: Could not restore geometry: {str(exc)}", file=sys.stderr)
        self.flowgraph_started = threading.Event()

        ##################################################
        # Variables
        ##################################################
        self.zmq_push_offset = zmq_push_offset = 1
        self.zmq_base_port = zmq_base_port = 20000
        self.samp_rate = samp_rate = int(1E6)
        self.radio_gain_db = radio_gain_db = 20
        self.enable_zmq = enable_zmq = 1
        self.enable_rx = enable_rx = 1
        self.enable_agc = enable_agc = 0
        self.center_frequency = center_frequency = 915E6

        ##################################################
        # Blocks
        ##################################################

        self._radio_gain_db_range = qtgui.Range(0, 40, 3, 20, 200)
        self._radio_gain_db_win = qtgui.RangeWidget(self._radio_gain_db_range, self.set_radio_gain_db, "Radio Gain (dB)", "counter_slider", float, QtCore.Qt.Horizontal)
        self.top_grid_layout.addWidget(self._radio_gain_db_win, 0, 2, 1, 1)
        for r in range(0, 1):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(2, 3):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._enable_zmq_choices = {'Pressed': 1, 'Released': 0}

        _enable_zmq_toggle_switch = qtgui.GrToggleSwitch(self.set_enable_zmq, f"Enable ZMQ Port {zmq_base_port+zmq_push_offset}", self._enable_zmq_choices, True, "green", "gray", 4, 50, 1, 1, self, 'value')
        self.enable_zmq = _enable_zmq_toggle_switch

        self.top_grid_layout.addWidget(_enable_zmq_toggle_switch, 0, 3, 1, 1)
        for r in range(0, 1):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(3, 4):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._enable_rx_choices = {'Pressed': 1, 'Released': 0}

        _enable_rx_toggle_switch = qtgui.GrToggleSwitch(self.set_enable_rx, 'Enable GUI', self._enable_rx_choices, True, "green", "gray", 4, 50, 1, 1, self, 'value')
        self.enable_rx = _enable_rx_toggle_switch

        self.top_grid_layout.addWidget(_enable_rx_toggle_switch, 0, 0, 1, 1)
        for r in range(0, 1):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(0, 1):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._center_frequency_msgdigctl_win = qtgui.MsgDigitalNumberControl(lbl='Carrier Frequency', min_freq_hz=902E6, max_freq_hz=928E6, parent=self, thousands_separator=",", background_color="black", fontColor="white", var_callback=self.set_center_frequency, outputmsgname='freq')
        self._center_frequency_msgdigctl_win.setValue(915E6)
        self._center_frequency_msgdigctl_win.setReadOnly(False)
        self.center_frequency = self._center_frequency_msgdigctl_win

        self.top_grid_layout.addWidget(self._center_frequency_msgdigctl_win, 0, 4, 1, 1)
        for r in range(0, 1):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(4, 5):
            self.top_grid_layout.setColumnStretch(c, 1)
        self.zeromq_push_sink_0_0 = zeromq.push_sink(gr.sizeof_gr_complex, 1, f"tcp://127.0.0.1:{zmq_base_port+zmq_push_offset}", 100, False, (-1), True)
        self.soapy_hackrf_source_0 = None
        dev = 'driver=hackrf'
        stream_args = ''
        tune_args = ['']
        settings = ['']

        self.soapy_hackrf_source_0 = soapy.source(dev, "fc32", 1, '',
                                  stream_args, tune_args, settings)
        self.soapy_hackrf_source_0.set_sample_rate(0, samp_rate)
        self.soapy_hackrf_source_0.set_bandwidth(0, 0)
        self.soapy_hackrf_source_0.set_frequency(0, center_frequency)
        self.soapy_hackrf_source_0.set_gain(0, 'AMP', True)
        self.soapy_hackrf_source_0.set_gain(0, 'LNA', min(max(radio_gain_db, 0.0), 40.0))
        self.soapy_hackrf_source_0.set_gain(0, 'VGA', min(max(16, 0.0), 62.0))
        self.qtgui_sink_x_0_0 = qtgui.sink_c(
            1024, #fftsize
            window.WIN_BLACKMAN_hARRIS, #wintype
            center_frequency, #fc
            samp_rate, #bw
            "ADALM Pluto Receiver", #name
            True, #plotfreq
            True, #plotwaterfall
            True, #plottime
            True, #plotconst
            None # parent
        )
        self.qtgui_sink_x_0_0.set_update_time(1.0/10)
        self._qtgui_sink_x_0_0_win = sip.wrapinstance(self.qtgui_sink_x_0_0.qwidget(), Qt.QWidget)

        self.qtgui_sink_x_0_0.enable_rf_freq(True)

        self.top_grid_layout.addWidget(self._qtgui_sink_x_0_0_win, 1, 0, 4, 5)
        for r in range(1, 5):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(0, 5):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._enable_agc_choices = {'Pressed': 0, 'Released': 1}

        _enable_agc_toggle_switch = qtgui.GrToggleSwitch(self.set_enable_agc, 'Enable AGC', self._enable_agc_choices, True, "green", "gray", 4, 50, 1, 1, self, 'value')
        self.enable_agc = _enable_agc_toggle_switch

        self.top_grid_layout.addWidget(_enable_agc_toggle_switch, 0, 1, 1, 1)
        for r in range(0, 1):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(1, 2):
            self.top_grid_layout.setColumnStretch(c, 1)
        self.blocks_selector_0_1 = blocks.selector(gr.sizeof_gr_complex*1,0,enable_rx)
        self.blocks_selector_0_1.set_enabled(True)
        self.blocks_selector_0_0_0 = blocks.selector(gr.sizeof_gr_complex*1,0,enable_zmq)
        self.blocks_selector_0_0_0.set_enabled(True)
        self.blocks_null_sink_0_1 = blocks.null_sink(gr.sizeof_gr_complex*1)
        self.blocks_null_sink_0_0_0 = blocks.null_sink(gr.sizeof_gr_complex*1)


        ##################################################
        # Connections
        ##################################################
        self.connect((self.blocks_selector_0_0_0, 0), (self.blocks_null_sink_0_0_0, 0))
        self.connect((self.blocks_selector_0_0_0, 1), (self.zeromq_push_sink_0_0, 0))
        self.connect((self.blocks_selector_0_1, 0), (self.blocks_null_sink_0_1, 0))
        self.connect((self.blocks_selector_0_1, 1), (self.qtgui_sink_x_0_0, 0))
        self.connect((self.soapy_hackrf_source_0, 0), (self.blocks_selector_0_0_0, 0))
        self.connect((self.soapy_hackrf_source_0, 0), (self.blocks_selector_0_1, 0))


    def closeEvent(self, event):
        self.settings = Qt.QSettings("gnuradio/flowgraphs", "harness_hackrf_rx")
        self.settings.setValue("geometry", self.saveGeometry())
        self.stop()
        self.wait()

        event.accept()

    def get_zmq_push_offset(self):
        return self.zmq_push_offset

    def set_zmq_push_offset(self, zmq_push_offset):
        self.zmq_push_offset = zmq_push_offset

    def get_zmq_base_port(self):
        return self.zmq_base_port

    def set_zmq_base_port(self, zmq_base_port):
        self.zmq_base_port = zmq_base_port

    def get_samp_rate(self):
        return self.samp_rate

    def set_samp_rate(self, samp_rate):
        self.samp_rate = samp_rate
        self.qtgui_sink_x_0_0.set_frequency_range(self.center_frequency, self.samp_rate)
        self.soapy_hackrf_source_0.set_sample_rate(0, self.samp_rate)

    def get_radio_gain_db(self):
        return self.radio_gain_db

    def set_radio_gain_db(self, radio_gain_db):
        self.radio_gain_db = radio_gain_db
        self.soapy_hackrf_source_0.set_gain(0, 'LNA', min(max(self.radio_gain_db, 0.0), 40.0))

    def get_enable_zmq(self):
        return self.enable_zmq

    def set_enable_zmq(self, enable_zmq):
        self.enable_zmq = enable_zmq
        self.blocks_selector_0_0_0.set_output_index(self.enable_zmq)

    def get_enable_rx(self):
        return self.enable_rx

    def set_enable_rx(self, enable_rx):
        self.enable_rx = enable_rx
        self.blocks_selector_0_1.set_output_index(self.enable_rx)

    def get_enable_agc(self):
        return self.enable_agc

    def set_enable_agc(self, enable_agc):
        self.enable_agc = enable_agc

    def get_center_frequency(self):
        return self.center_frequency

    def set_center_frequency(self, center_frequency):
        self.center_frequency = center_frequency
        self.qtgui_sink_x_0_0.set_frequency_range(self.center_frequency, self.samp_rate)
        self.soapy_hackrf_source_0.set_frequency(0, self.center_frequency)




def main(top_block_cls=harness_hackrf_rx, options=None):

    qapp = Qt.QApplication(sys.argv)

    tb = top_block_cls()

    tb.start()
    tb.flowgraph_started.set()

    tb.show()

    def sig_handler(sig=None, frame=None):
        tb.stop()
        tb.wait()

        Qt.QApplication.quit()

    signal.signal(signal.SIGINT, sig_handler)
    signal.signal(signal.SIGTERM, sig_handler)

    timer = Qt.QTimer()
    timer.start(500)
    timer.timeout.connect(lambda: None)

    qapp.exec_()

if __name__ == '__main__':
    main()
